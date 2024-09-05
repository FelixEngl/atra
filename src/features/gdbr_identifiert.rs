use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use ego_tree::iter::{Edge, Traverse};
use ego_tree::NodeRef;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use liblinear::solver::traits::{IsTrainableSolver, Solver};
use liblinear::Model;
use liblinear::solver::{GenericSolver, L2R_L2LOSS_SVR};
use scraper::{Html, Node};
use crate::features::html_tags::{HtmlTag, HtmlTagCategory};
use crate::features::scraper_ext::Text;
use crate::features::svm::classifier::DocumentClassifier;
use crate::features::text_processing::tf_idf::{Idf, IdfAlgorithm, Tf, TfAlgorithm};

#[derive(Serialize, Deserialize, Debug)]
#[serde(bound(
    serialize = "TF: Serialize, IDF: Serialize, SOLVER: IsTrainableSolver",
    deserialize = "TF: DeserializeOwned, IDF: DeserializeOwned, SOLVER: IsTrainableSolver, Model<SOLVER>: TryFrom<Model<GenericSolver>>"
))]
pub struct GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {
    solver: DocumentClassifier<TF, IDF, SOLVER>,
    element_threshold: f64,
    gdbr_threshold: f64,
    filter_on_max_score: bool
}

impl<TF, IDF, SOLVER> GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {
    pub fn new(solver: DocumentClassifier<TF, IDF, SOLVER>, element_threshold: f64, gdbr_threshold: f64, filter_on_max_score: bool) -> Self {
        Self { solver, element_threshold, gdbr_threshold, filter_on_max_score }
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
                (!result.is_nan() && result >= self.element_threshold).then_some((result, node))
            }
            Node::Element(element) => {
                let values = Text::traverse(&node).join(" ");
                let result = self.predict(&values).unwrap();
                (!result.is_nan() && result >= self.element_threshold).then_some((result, node))
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
                        Entry::Occupied(mut entry) => {
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

    pub fn has_gbr(&self, html: &str) -> bool {
        let html = Html::parse_document(html);
        if let Some(gdbr_nodes) = self.identify_gdbr_elements_in_html(&html) {
            gdbr_nodes.last().is_some_and(|value| {
                debug_assert!(!value.is_empty());
                println!("Result: {}", value.iter().map(|value| format!("({}/{})", value.score(), value.max_score())).join(", "));
                if self.filter_on_max_score {
                    value.iter().any(|value| value.max_score() >= self.gdbr_threshold)
                } else {
                    value.iter().any(|value| value.score() >= self.gdbr_threshold)
                }
            })
        } else {
            false
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::hash::{Hash};
    use std::io::BufReader;
    use std::ops::Deref;
    use isolang::Language;
    use itertools::Itertools;
    use scraper::{Html, Node};
    use serde::{Deserialize, Serialize};
    use crate::core::url::atra_uri::AtraUri;
    use crate::features::gdbr_identifiert::GdbrIdentifier;
    use crate::features::scraper_ext::Text;
    use crate::features::svm::test::{create_german_gdbr_svm, train_data};

    #[derive(Deserialize)]
    struct TestSet<T> {
        rows: Vec<T>
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct TestSetRow {
        language: String,
        url: String,
        content: String,
        page_source_html: String,
        content_removed: Option<String>,
        page_source_cleaned_html: Option<String>,
        page_source_removed_html: Option<String>,
        #[serde(alias = "contains_GDPR")]
        contains_gdbr: bool,
    }

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct ProcessedTestSetRow {
        language: String,
        url: String,
        content: String,
        page_source_html: String,
        content_removed: Option<String>,
        page_source_cleaned_html: Option<String>,
        page_source_removed_html: Option<String>,
        #[serde(alias = "contains_GDPR")]
        contains_gdbr: bool,
        content_removed_assistant: Option<String>,
        page_source_cleaned_assistant: Option<String>
    }

    #[derive(Serialize)]
    struct TestEntryRow {
        has_gdbr: bool,
        language: Language,
        uri: AtraUri,
        content: String,
        html: String,
        removed_html_part: Option<String>
    }

    #[test]
    fn create(){
        let test_set: TestSet<TestSetRow> = serde_json::from_reader(BufReader::new(File::open("D:\\Downloads\\processed_test_set.json").unwrap())).unwrap();
        for value in test_set.rows {
            let language = match value.language.as_str() {
                "__label__de" => {
                    Language::Deu
                }
                _ => unreachable!()
            };
            let uri: AtraUri = value.url.parse().unwrap();
            println!("{}", uri);
            println!("{}", value.content);
            println!("");
        }
    }

    #[test]
    fn test_might() {
        const DATA: &'static str = include_str!("../core/samples/Amazon.html");

        let identifier = GdbrIdentifier::new(
            create_german_gdbr_svm(),
            0.1,
            0.5,
            false
        );
        let html = Html::parse_document(DATA);
        let gdbr_nodes = identifier.identify_gdbr_elements_in_html(&html).unwrap();

        for (i, v) in gdbr_nodes.into_iter().enumerate() {
            println!("Level: {i}");
            for value in v {
                match value.node().value() {
                    Node::Text(value) => {println!("    Text")}
                    Node::Element(value) => {println!("    Element: {}", value.name())}
                    _ => println!("    Unsupported Type")
                }
                let mut content = match value.node().value() {
                    Node::Text(value) => value.deref().to_string(),
                    Node::Element(elem) => Text::traverse(&value.node()).join(" "),
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
            false
        );
        for value in train_data() {
            let result = identifier.has_gbr(&value.text);
            if result != value.is_class {
                println!("{result} || {} :: {}", value.is_class, value.text);
            }
        }
    }
}