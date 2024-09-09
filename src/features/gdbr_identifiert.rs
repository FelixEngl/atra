use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use ego_tree::NodeRef;
use isolang::Language;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use liblinear::solver::traits::{IsTrainableSolver, Solver};
use liblinear::Model;
use liblinear::parameter::serde::SupportsParametersCreation;
use liblinear::solver::{GenericSolver};
use scraper::{Html, Node};
use crate::core::io::root::RootSetter;
use crate::features::html_tags::{HtmlTag, HtmlTagCategory};
use crate::features::scraper_ext::Text;
use crate::features::svm::classifier::DocumentClassifier;
use crate::features::svm::config::SvmRecognizerConfig;
use crate::features::svm::{create_document_classifier};
use crate::features::svm::error::SvmCreationError;
use crate::features::text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm};
use crate::features::tokenizing::stopwords::StopWordRegistry;
use crate::features::tokenizing::SupportsStopwords;


pub struct InitHelper<'a, TF: TfAlgorithm, IDF: IdfAlgorithm, R: RootSetter> {
    pub gdbr_config: Option<&'a GdbrIdentifierRegistryConfig<TF, IDF>>,
    pub root_setter: Option<&'a R>,
    pub stop_word_registry: Option<&'a StopWordRegistry>,
}

impl<'a, TF: TfAlgorithm, IDF: IdfAlgorithm, R: RootSetter> SupportsGdbrIdentifier<TF, IDF> for InitHelper<'a, TF, IDF, R> {
    fn gdbr_config(&self) -> Option<&GdbrIdentifierRegistryConfig<TF, IDF>> {
        self.gdbr_config
    }

    fn root_setter(&self) -> Option<&impl RootSetter> {
        self.root_setter
    }
}

impl<'a, TF: TfAlgorithm, IDF: IdfAlgorithm, R: RootSetter> SupportsStopwords for InitHelper<'a, TF, IDF, R> {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        self.stop_word_registry
    }
}

// L2R_L2LOSS_SVR

/// A trait that allows a context to support the initialisation of gdbr
pub trait SupportsGdbrIdentifier<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    fn gdbr_config(&self) -> Option<&GdbrIdentifierRegistryConfig<TF, IDF>>;

    fn root_setter(&self) -> Option<&impl RootSetter>;
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(bound(
    serialize = "TF: Clone + Serialize + Debug, IDF: Clone + Serialize + Debug",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
pub struct GdbrIdentifierRegistryConfig<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    default: Option<GdbrIdentifierConfig<TF, IDF>>,
    by_language: Option<HashMap<Language, LanguageBoundGdbrIdentifierConfig<TF, IDF>>>
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(bound(
    serialize = "TF: Clone + Serialize + Debug, IDF: Clone + Serialize + Debug",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
pub struct LanguageBoundGdbrIdentifierConfig<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    only_if_reliable: bool,
    identifier: GdbrIdentifierConfig<TF, IDF>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(bound(
    serialize = "TF: Clone + Serialize + Debug, IDF: Clone + Serialize + Debug",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
pub struct GdbrIdentifierConfig<TF: TfAlgorithm, IDF: IdfAlgorithm>  {
    pub threshold: f64,
    pub filter_threshold: f64,
    pub filter_by: FilterMode,
    pub svm: SvmRecognizerConfig<TF, IDF>
}

impl<TF: TfAlgorithm + PartialEq, IDF: IdfAlgorithm + PartialEq> Eq for GdbrIdentifierConfig<TF, IDF> {}

impl<TF: TfAlgorithm + PartialEq, IDF: IdfAlgorithm + PartialEq> PartialEq for GdbrIdentifierConfig<TF, IDF> {
    fn eq(&self, other: &Self) -> bool {
        self.filter_by.eq(&other.filter_by)
            && float_cmp::approx_eq!(f64, self.filter_threshold, other.filter_threshold)
            && float_cmp::approx_eq!(f64, self.threshold, other.threshold)
            && self.svm == other.svm
    }
}



#[derive(Debug, Default)]
pub struct GdbrIdentifierRegistry<TF, IDF, SOLVER: Solver> {
    default: Option<GdbrIdentifier<TF, IDF, SOLVER>>,
    by_language: Option<HashMap<Language, LanguageBoundGdbrIdentifier<TF, IDF, SOLVER>>>
}

impl<TF, IDF, SOLVER: Solver> GdbrIdentifierRegistry<TF, IDF, SOLVER> {
    pub fn get_by_language(&self, language: &whatlang::Info) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        let by_language = self.by_language.as_ref()?;
        let identified = Language::from_639_3(language.lang().code())?;
        let found = by_language.get(&identified)?;
        found.get_with_reliability(language.is_reliable())
    }

    pub fn get_default(&self) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        self.default.as_ref()
    }

    pub fn get_by_language_or_default(&self, language: &whatlang::Info) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        match self.get_by_language(language) {
            x @ Some(_) => x,
            None => self.get_default()
        }
    }
}

impl<TF, IDF, SOLVER: Solver> GdbrIdentifierRegistry<TF, IDF, SOLVER>
where
    TF: TfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    IDF: IdfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    SOLVER: SupportsParametersCreation,
    Model<SOLVER>: TryFrom<Model<GenericSolver>>
{

    pub fn new_from_config<C: SupportsGdbrIdentifier<TF, IDF> + SupportsStopwords>(context: &C) -> Result<Option<Self>, SvmCreationError<IDF>> {
        if let Some(config) = context.gdbr_config() {
            let default = if let Some(ref default) = config.default {
                match create_document_classifier(&default.svm, context, context.root_setter()) {
                    Ok(value) => {
                        Some(
                            GdbrIdentifier::new(
                                value,
                                default.threshold,
                                default.filter_threshold,
                                default.filter_by
                            )
                        )
                    }
                    Err(err) => {
                        return Err(err)
                    }
                }
            } else {
                None
            };

            let by_language = if let Some(ref others) = config.by_language {
                  others.iter().map( |(k, v)| {
                      match create_document_classifier(&v.identifier.svm, context, context.root_setter()) {
                          Ok(value) => {
                              Ok(
                                  (*k, LanguageBoundGdbrIdentifier::new(
                                      v.only_if_reliable,
                                      GdbrIdentifier::new(
                                          value,
                                          v.identifier.threshold,
                                          v.identifier.filter_threshold,
                                          v.identifier.filter_by
                                      )
                                  ))
                              )
                          }
                          Err(err) => {
                              Err(err)
                          }
                      }
                  }).process_results(|value| {
                      let collected = value.collect::<HashMap<Language, LanguageBoundGdbrIdentifier<_, _, _>>>();
                      (!collected.is_empty()).then_some(collected)
                  })?
            } else {
                None
            };

            Ok(
                Some(
                    Self {
                        default,
                        by_language
                    }
                )
            )
        } else {
            Ok(None)
        }
    }

}

#[derive(Debug)]
struct LanguageBoundGdbrIdentifier<TF, IDF, SOLVER: Solver> {
    only_if_reliable: bool,
    identifier: GdbrIdentifier<TF, IDF, SOLVER>
}

impl<TF, IDF, SOLVER: Solver> LanguageBoundGdbrIdentifier<TF, IDF, SOLVER> {
    pub fn new(only_if_reliable: bool, identifier: GdbrIdentifier<TF, IDF, SOLVER>) -> Self {
        Self { only_if_reliable, identifier }
    }

    pub fn get_with_reliability(&self, is_reliable: bool) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>>
    {
        if !self.only_if_reliable || is_reliable {
            Some(self.get())
        } else {
            None
        }
    }

    pub fn get(&self) -> &GdbrIdentifier<TF, IDF, SOLVER>
    {
        &self.identifier
    }
}


#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum FilterMode {
    #[default]
    OnScore,
    OnMaxScore,
    OnAverageScore,
}

impl FilterMode {
    pub fn is_above_threshold<'a, T>(&self, score: &ScoredNodeRef<'a, T>, threshold: f64) -> bool {
        match self {
            FilterMode::OnScore => {
                score.score() >= threshold
            }
            FilterMode::OnMaxScore => {
                score.max_score() >= threshold
            }
            FilterMode::OnAverageScore => {
                score.avg_score() >= threshold
            }
        }
    }

    pub fn find_all_above<'a, T: 'a, I: IntoIterator<Item=ScoredNodeRef<'a, T>>>(&self, scores: I, threshold: f64) -> Vec<I::Item> {
        match self {
            FilterMode::OnScore => {
                scores.into_iter().filter(|value: &ScoredNodeRef<'a, T>| value.score() >= threshold).collect_vec()
            }
            FilterMode::OnMaxScore => {
                scores.into_iter().filter(|value: &ScoredNodeRef<'a, T>| value.max_score() >= threshold).collect_vec()
            }
            FilterMode::OnAverageScore => {
                scores.into_iter().filter(|value: &ScoredNodeRef<'a, T>| value.avg_score() >= threshold).collect_vec()
            }
        }
    }
}



#[derive(Serialize, Deserialize, Debug)]
#[serde(bound(
    serialize = "TF: Serialize, IDF: Serialize, SOLVER: IsTrainableSolver",
    deserialize = "TF: DeserializeOwned, IDF: DeserializeOwned, SOLVER: IsTrainableSolver, Model<SOLVER>: TryFrom<Model<GenericSolver>>"
))]
pub struct GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {
    solver: DocumentClassifier<TF, IDF, SOLVER>,
    threshold: f64,
    filter_threshold: f64,
    filter_by: FilterMode
}

unsafe impl<TF, IDF, SOLVER> Sync for GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver{}
unsafe impl<TF, IDF, SOLVER> Send for GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver{}

impl<TF, IDF, SOLVER> GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {
    pub fn new(solver: DocumentClassifier<TF, IDF, SOLVER>, threshold: f64, filter_score: f64, filter_by: FilterMode) -> Self {
        Self { solver, threshold, filter_threshold: filter_score, filter_by }
    }
}

impl<TF, IDF, SOLVER> Deref for GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {
    type Target = DocumentClassifier<TF, IDF, SOLVER> ;

    fn deref(&self) -> &Self::Target {
        &self.solver
    }
}


#[derive(Clone)]
#[repr(transparent)]
pub struct ScoredNodeRef<'a, T> {
    inner: Rc<(f64, Cell<f64>, NodeRef<'a, T>)>
}
impl<'a, T>  ScoredNodeRef<'a, T> {
    pub fn new(inner: (f64, Cell<f64>, NodeRef<'a, T>)) -> Self {
        Self { inner: Rc::new(inner) }
    }

    pub fn score(&self) -> f64 {
        self.inner.0
    }

    pub fn max_score(&self) -> f64 {
        self.inner.1.get()
    }

    #[inline(always)]
    pub fn avg_score(&self) -> f64 {
        (self.inner.0 + self.inner.1.get()) / 2.0
    }

    pub fn set_max_score(&mut self, max_score: f64) {
        let max = self.inner.1.get().max(max_score);
        self.inner.1.replace(max);
    }

    pub fn node(&self) -> &NodeRef<'a, T> {
        &self.inner.2
    }
}
impl<'a, T> From<(f64, NodeRef<'a, T>)> for ScoredNodeRef<'a, T> {
    fn from((score, node): (f64, NodeRef<'a, T>)) -> Self {
        Self::new((score, Cell::new(score), node))
    }
}
impl<'a, T> Eq for ScoredNodeRef<'a, T>{}
impl<'a, T> PartialEq for ScoredNodeRef<'a, T>  {
    fn eq(&self, other: &Self) -> bool {
        self.inner.2.eq(&other.inner.2)
    }
}
impl<'a, T> Hash for ScoredNodeRef<'a, T>  {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.2.id().hash(state)
    }
}


impl<TF, IDF, SOLVER> GdbrIdentifier<TF, IDF, SOLVER>
where
    TF: TfAlgorithm,
    IDF: IdfAlgorithm,
    SOLVER: Solver
{
    fn is_possibly_gdbr_text_containing_element(node: &NodeRef<Node>) -> bool {
        match node.value() {
            Node::Element(element) => {
                match element.name().parse::<HtmlTag>() {
                    Ok(value) => {
                        match value {
                            HtmlTag::P | HtmlTag::A | HtmlTag::Div
                            | HtmlTag::Title | HtmlTag::Dialog | HtmlTag::Details => true,
                            otherwise => !matches!(
                                otherwise.category(),
                                HtmlTagCategory::Programming
                                | HtmlTagCategory::StylesAndSemantics
                            )
                        }
                    },
                    Err(err) => {
                        log::warn!("Was not able to identify {} as a tag! Treat is as true...", err.0);
                        true
                    }
                }
            },
            Node::Text(_) => {
                if let Some(parent) = node.parent() {
                    matches!(parent.value(), Node::Element(_)) && Self::is_possibly_gdbr_text_containing_element(&parent)
                } else {
                    true
                }
            },
            _ => false
        }
    }

    fn filter_fkt_without_type_filter<'a>(&self, node: NodeRef<'a, Node>) -> Option<(f64, NodeRef<'a, Node>)> {
        match node.value() {
            Node::Text(text) => {
                let result = self.predict(text.deref()).unwrap();
                (!result.is_nan() && result >= self.threshold).then_some((result, node))
            }
            Node::Element(_) => {
                let values = Text::traverse(&node).join(" ");
                let result = self.predict(&values).unwrap();
                (!result.is_nan() && result >= self.threshold).then_some((result, node))
            }
            _ => None
        }
    }

    fn filter_fkt<'a>(&self, node: NodeRef<'a, Node>) -> Option<(f64, NodeRef<'a, Node>)> {
        if Self::is_possibly_gdbr_text_containing_element(&node) {
            self.filter_fkt_without_type_filter(node)
        } else {
            None
        }
    }

    fn identify_gdbr_elements_in_html<'a>(&self, html: &'a Html) -> Option<Vec<Vec<ScoredNodeRef<'a, Node>>>> {
        let initial = html.tree
            .nodes()
            .into_iter()
            .filter(|value| {
                !value.has_children() && if let Some(parent) = value.parent() {
                    if let Some(elem) = parent.value().as_element() {
                        if let Ok(tag) = elem.name().parse::<HtmlTag>() {
                            !matches!(tag.category(),HtmlTagCategory::Programming| HtmlTagCategory::StylesAndSemantics)
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .filter_map(|node| self.filter_fkt_without_type_filter(node).map(ScoredNodeRef::from))
            .collect_vec();

        if initial.is_empty() {
            return None
        }

        let mut visited: HashMap<_, _> = initial.iter().map(|value| (value.node().id(), value.clone())).collect();

        let mut result_collection: Vec<Vec<ScoredNodeRef<Node>>> = Vec::new();
        result_collection.push(initial);

        loop {
            let mut result = HashSet::new();
            for last_entry in result_collection.last().unwrap() {
                if let Some(parent) = last_entry.node().parent() {
                    match visited.entry(parent.id()) {
                        Entry::Vacant(entry) => {
                            if let Some(value) = self.filter_fkt(parent) {
                                let mut value = ScoredNodeRef::from(value);
                                value.set_max_score(last_entry.max_score());
                                entry.insert(value.clone());
                                result.insert(value);
                            }
                        }
                        Entry::Occupied(entry) => {
                            let mut v = entry.get().clone();
                            v.set_max_score(last_entry.max_score());
                            result.insert(v);
                        }
                    }
                }
            }

            match result.len() {
                0 => {},
                1 => result_collection.push(Vec::from_iter(result)),
                _ => {
                    result_collection.push(Vec::from_iter(result));
                    continue
                },
            }
            break Some(result_collection)
        }
    }

    fn get_most_probable<'a>(&self, html: &'a Html) -> Option<ScoredNodeRef<'a, Node>> {
        if let Some(gdbr_nodes) = self.identify_gdbr_elements_in_html(html) {
            let value = VecDeque::from(gdbr_nodes).pop_back()?;
            debug_assert!(!value.is_empty());
            self.filter_by.find_all_above(value, self.filter_threshold).into_iter().next()
        } else {
            None
        }
    }

    /// Removes the gbr from the parsed html
    pub fn remove_gdbr(&self, html: &mut Html) {
        if let Some(found) = self.get_most_probable(&html) {
            let mut node = unsafe{html.tree.get_unchecked_mut(found.node().id())};
            node.detach()
        }
    }

    pub fn has_gbr(&self, html: &str) -> bool {
        let html = Html::parse_document(html);
        self.get_most_probable(&html).is_some()
    }
}

#[cfg(test)]
mod test {
    use std::ops::Deref;
    use itertools::Itertools;
    use scraper::{Html, Node};
    use crate::features::gdbr_identifiert::{FilterMode, GdbrIdentifier};
    use crate::features::scraper_ext::Text;
    use crate::features::svm::test::{create_german_gdbr_svm, train_data};


    #[test]
    fn test_might() {
        const DATA: &'static str = include_str!("../core/samples/Amazon.html");

        let identifier = GdbrIdentifier::new(
            create_german_gdbr_svm(),
            0.1,
            0.5,
            FilterMode::OnMaxScore
        );
        let html = Html::parse_document(DATA);
        let gdbr_nodes = identifier.identify_gdbr_elements_in_html(&html).unwrap();

        for (i, v) in gdbr_nodes.into_iter().enumerate() {
            println!("Level: {i}");
            for value in v {
                match value.node().value() {
                    Node::Text(_) => {println!("    Text")}
                    Node::Element(value) => {println!("    Element: {}", value.name())}
                    _ => println!("    Unsupported Type")
                }
                let mut content = match value.node().value() {
                    Node::Text(value) => value.deref().to_string(),
                    Node::Element(_) => Text::traverse(&value.node()).join(" "),
                    _ => {
                        println!(">> ERROR with node!");
                        continue
                    }
                };
                let mut result = identifier.tokenize(&content).into_iter().join(", ");
                content.truncate(100);
                result.truncate(100);
                println!("    {} ({}) - {content} ({})\n", value.score(), value.max_score(), result);
            }
        }
    }

    #[test]
    fn test_with_traindata(){
        let identifier = GdbrIdentifier::new(
            create_german_gdbr_svm(),
            0.1,
            0.5,
            FilterMode::OnScore
        );
        for value in train_data() {
            let result = identifier.has_gbr(&value.text);
            if result != value.is_class {
                println!("{result} || {} :: {}", value.is_class, value.text);
            }
        }
    }
}