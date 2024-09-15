// Copyright 2024 Felix Engl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::contexts::traits::SupportsStopwordsRegistry;
use crate::contexts::BaseContext;
use crate::gdbr::scraper_ext::Text;
use crate::html::{HtmlTag, HtmlTagCategory};
use crate::toolkit::LanguageInformation;
#[cfg(test)]
use camino::Utf8Path;
use ego_tree::NodeRef;
use isolang::Language;
use itertools::Itertools;
use liblinear::parameter::serde::SupportsParametersCreation;
use liblinear::solver::traits::{IsTrainableSolver, Solver};
use liblinear::solver::GenericSolver;
use liblinear::Model;
use scraper::{Html, Node};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use svm::classifier::DocumentClassifier;
use svm::config::SvmRecognizerConfig;
use svm::create_document_classifier;
use svm::error::SvmCreationError;
use text_processing::stopword_registry::StopWordRegistry;
use text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm};

pub struct InitHelper<'a, TF: TfAlgorithm, IDF: IdfAlgorithm> {
    pub gdbr_config: Option<&'a GdbrIdentifierRegistryConfig<TF, IDF>>,
    pub stop_word_registry: Option<&'a StopWordRegistry>,
}

impl<'a, TF: TfAlgorithm, IDF: IdfAlgorithm> GdbrIdentifierCreationContext<TF, IDF>
    for InitHelper<'a, TF, IDF>
{
    fn gdbr_config(&self) -> Option<&GdbrIdentifierRegistryConfig<TF, IDF>> {
        self.gdbr_config
    }
}

impl<'a, TF: TfAlgorithm, IDF: IdfAlgorithm> BaseContext for InitHelper<'a, TF, IDF> {}

impl<'a, TF: TfAlgorithm, IDF: IdfAlgorithm> SupportsStopwordsRegistry for InitHelper<'a, TF, IDF> {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        self.stop_word_registry
    }
}

// L2R_L2LOSS_SVR

/// A trait that allows a context to support the initialisation of gdbr
pub trait GdbrIdentifierCreationContext<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    fn gdbr_config(&self) -> Option<&GdbrIdentifierRegistryConfig<TF, IDF>>;
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(bound(
    serialize = "TF: Clone + Serialize + Debug, IDF: Clone + Serialize + Debug",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
pub struct GdbrIdentifierRegistryConfig<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    pub default: Option<GdbrIdentifierConfig<TF, IDF>>,
    pub by_language: Option<HashMap<Language, LanguageBoundGdbrIdentifierConfig<TF, IDF>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(bound(
    serialize = "TF: Clone + Serialize + Debug, IDF: Clone + Serialize + Debug",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
pub struct LanguageBoundGdbrIdentifierConfig<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    #[serde(default = "_default_required_reliability")]
    pub required_reliability: f64,
    pub identifier: GdbrIdentifierConfig<TF, IDF>,
}

fn _default_required_reliability() -> f64 {
    0.9
}

impl<TF: TfAlgorithm, IDF: IdfAlgorithm> Eq for LanguageBoundGdbrIdentifierConfig<TF, IDF>
where
    TF: Eq,
    IDF: Eq,
{
}
impl<TF: TfAlgorithm, IDF: IdfAlgorithm> PartialEq for LanguageBoundGdbrIdentifierConfig<TF, IDF>
where
    TF: PartialEq,
    IDF: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.identifier.eq(&other.identifier)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(bound(
    serialize = "TF: Clone + Serialize + Debug, IDF: Clone + Serialize + Debug",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
pub struct GdbrIdentifierConfig<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    pub threshold: f64,
    pub filter_threshold: f64,
    pub filter_by: FilterMode,
    pub svm: SvmRecognizerConfig<TF, IDF>,
}

impl<TF: TfAlgorithm + PartialEq, IDF: IdfAlgorithm + PartialEq> Eq
    for GdbrIdentifierConfig<TF, IDF>
{
}

impl<TF: TfAlgorithm + PartialEq, IDF: IdfAlgorithm + PartialEq> PartialEq
    for GdbrIdentifierConfig<TF, IDF>
{
    fn eq(&self, other: &Self) -> bool {
        self.filter_by.eq(&other.filter_by)
            && float_cmp::approx_eq!(f64, self.filter_threshold, other.filter_threshold)
            && float_cmp::approx_eq!(f64, self.threshold, other.threshold)
            && self.svm == other.svm
    }
}

pub trait GdbrRegistry {
    type TF: TfAlgorithm;
    type IDF: IdfAlgorithm;
    type SOLVER: Solver;

    fn get_default(&self) -> Option<&GdbrIdentifier<Self::TF, Self::IDF, Self::SOLVER>>;
    fn get_by_language(
        &self,
        language: &LanguageInformation,
    ) -> Option<&GdbrIdentifier<Self::TF, Self::IDF, Self::SOLVER>>;
    fn get_by_language_or_default(
        &self,
        language: Option<&LanguageInformation>,
    ) -> Option<&GdbrIdentifier<Self::TF, Self::IDF, Self::SOLVER>>;
}

#[derive(Debug, Default)]
pub struct GdbrIdentifierRegistry<TF, IDF, SOLVER: Solver> {
    default: Option<GdbrIdentifier<TF, IDF, SOLVER>>,
    by_language: Option<HashMap<Language, LanguageBoundGdbrIdentifier<TF, IDF, SOLVER>>>,
}

impl<TF, IDF, SOLVER> GdbrRegistry for GdbrIdentifierRegistry<TF, IDF, SOLVER>
where
    TF: TfAlgorithm,
    IDF: IdfAlgorithm,
    SOLVER: Solver,
{
    type TF = TF;
    type IDF = IDF;
    type SOLVER = SOLVER;

    fn get_by_language(
        &self,
        language: &LanguageInformation,
    ) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        let by_language = self.by_language.as_ref()?;
        let found = by_language.get(&language.lang())?;
        found.get_with_reliability(language.confidence())
    }

    fn get_default(&self) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        self.default.as_ref()
    }

    fn get_by_language_or_default(
        &self,
        language: Option<&LanguageInformation>,
    ) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        if let Some(language) = language {
            match self.get_by_language(language) {
                x @ Some(_) => x,
                None => self.get_default(),
            }
        } else {
            self.get_default()
        }
    }
}

impl<TF, IDF, SOLVER: Solver> GdbrIdentifierRegistry<TF, IDF, SOLVER>
where
    TF: TfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    IDF: IdfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    SOLVER: SupportsParametersCreation,
    Model<SOLVER>: TryFrom<Model<GenericSolver>>,
{
    /// Initialize the GdbrIdentifierRegistry from a config for a given context
    pub fn new_from_config<
        C: GdbrIdentifierCreationContext<TF, IDF> + SupportsStopwordsRegistry,
    >(
        context: &C,
    ) -> Result<Option<Self>, SvmCreationError<IDF>> {
        if let Some(config) = context.gdbr_config() {
            let default = if let Some(ref default) = config.default {
                match create_document_classifier(&default.svm, context.stopword_registry()) {
                    Ok(value) => Some(GdbrIdentifier::new(
                        value,
                        default.threshold,
                        default.filter_threshold,
                        default.filter_by,
                    )),
                    Err(err) => return Err(err),
                }
            } else {
                None
            };

            let by_language = if let Some(ref others) = config.by_language {
                others
                    .iter()
                    .map(|(k, v)| {
                        match create_document_classifier(
                            &v.identifier.svm,
                            context.stopword_registry(),
                        ) {
                            Ok(value) => Ok((
                                *k,
                                LanguageBoundGdbrIdentifier::new(
                                    v.required_reliability,
                                    GdbrIdentifier::new(
                                        value,
                                        v.identifier.threshold,
                                        v.identifier.filter_threshold,
                                        v.identifier.filter_by,
                                    ),
                                ),
                            )),
                            Err(err) => Err(err),
                        }
                    })
                    .process_results(|value| {
                        let collected = value
                            .collect::<HashMap<Language, LanguageBoundGdbrIdentifier<_, _, _>>>();
                        (!collected.is_empty()).then_some(collected)
                    })?
            } else {
                None
            };

            log::info!("Finished creating gdbr identifiers.");
            Ok(Some(Self {
                default,
                by_language,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
struct LanguageBoundGdbrIdentifier<TF, IDF, SOLVER: Solver> {
    reliable_threshold: f64,
    identifier: GdbrIdentifier<TF, IDF, SOLVER>,
}

impl<TF, IDF, SOLVER: Solver> LanguageBoundGdbrIdentifier<TF, IDF, SOLVER> {
    pub fn new(reliable_threshold: f64, identifier: GdbrIdentifier<TF, IDF, SOLVER>) -> Self {
        Self {
            reliable_threshold,
            identifier,
        }
    }

    pub fn get_with_reliability(
        &self,
        reliability: f64,
    ) -> Option<&GdbrIdentifier<TF, IDF, SOLVER>> {
        if reliability < self.reliable_threshold {
            Some(self.get())
        } else {
            None
        }
    }

    pub fn get(&self) -> &GdbrIdentifier<TF, IDF, SOLVER> {
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
    #[cfg(test)]
    pub fn is_above_threshold<'a, T>(&self, score: &ScoredNodeRef<'a, T>, threshold: f64) -> bool {
        match self {
            FilterMode::OnScore => score.score() >= threshold,
            FilterMode::OnMaxScore => score.max_score() >= threshold,
            FilterMode::OnAverageScore => score.avg_score() >= threshold,
        }
    }

    #[cfg(test)]
    pub fn find_all_above<'a, T: 'a, I: IntoIterator<Item = ScoredNodeRef<'a, T>>>(
        &self,
        scores: I,
        threshold: f64,
    ) -> Vec<I::Item> {
        match self {
            FilterMode::OnScore => scores
                .into_iter()
                .filter(|value: &ScoredNodeRef<'a, T>| value.score() >= threshold)
                .collect_vec(),
            FilterMode::OnMaxScore => scores
                .into_iter()
                .filter(|value: &ScoredNodeRef<'a, T>| value.max_score() >= threshold)
                .collect_vec(),
            FilterMode::OnAverageScore => scores
                .into_iter()
                .filter(|value: &ScoredNodeRef<'a, T>| value.avg_score() >= threshold)
                .collect_vec(),
        }
    }

    pub fn find_max_by<'a, T: 'a, I: IntoIterator<Item = ScoredNodeRef<'a, T>>>(
        &self,
        scores: I,
        threshold: f64,
    ) -> Option<I::Item> {
        match self {
            FilterMode::OnScore => scores
                .into_iter()
                .filter(|value: &ScoredNodeRef<'a, T>| value.score() >= threshold)
                .max_by(|a, b| a.score().total_cmp(&b.score())),
            FilterMode::OnMaxScore => scores
                .into_iter()
                .filter(|value: &ScoredNodeRef<'a, T>| value.max_score() >= threshold)
                .max_by(|a, b| a.max_score().total_cmp(&b.max_score())),
            FilterMode::OnAverageScore => scores
                .into_iter()
                .filter(|value: &ScoredNodeRef<'a, T>| value.avg_score() >= threshold)
                .max_by(|a, b| a.avg_score().total_cmp(&b.avg_score())),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(bound(
    serialize = "TF: Serialize, IDF: Serialize, SOLVER: IsTrainableSolver",
    deserialize = "TF: DeserializeOwned, IDF: DeserializeOwned, SOLVER: IsTrainableSolver, Model<SOLVER>: TryFrom<Model<GenericSolver>>"
))]
pub struct GdbrIdentifier<TF, IDF, SOLVER>
where
    SOLVER: Solver,
{
    solver: DocumentClassifier<TF, IDF, SOLVER>,
    #[serde(default = "_threshold_default")]
    threshold: f64,
    #[serde(default = "_filter_threshold_default")]
    filter_threshold: f64,
    #[serde(default = "FilterMode::default")]
    filter_by: FilterMode,
}

fn _threshold_default() -> f64 {
    0.1
}

fn _filter_threshold_default() -> f64 {
    0.5
}

unsafe impl<TF, IDF, SOLVER> Sync for GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {}
unsafe impl<TF, IDF, SOLVER> Send for GdbrIdentifier<TF, IDF, SOLVER> where SOLVER: Solver {}

impl<TF, IDF, SOLVER> GdbrIdentifier<TF, IDF, SOLVER>
where
    SOLVER: Solver,
{
    pub fn new(
        solver: DocumentClassifier<TF, IDF, SOLVER>,
        threshold: f64,
        filter_score: f64,
        filter_by: FilterMode,
    ) -> Self {
        Self {
            solver,
            threshold,
            filter_threshold: filter_score,
            filter_by,
        }
    }
}

impl<TF, IDF, SOLVER> Deref for GdbrIdentifier<TF, IDF, SOLVER>
where
    SOLVER: Solver,
{
    type Target = DocumentClassifier<TF, IDF, SOLVER>;

    fn deref(&self) -> &Self::Target {
        &self.solver
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct ScoredNodeRef<'a, T> {
    inner: Rc<(f64, Cell<f64>, NodeRef<'a, T>)>,
}
impl<'a, T> ScoredNodeRef<'a, T> {
    #[cfg(test)]
    pub fn new(score: f64, max_score: f64, node: NodeRef<'a, T>) -> Self {
        Self {
            inner: Rc::new((score, Cell::new(max_score), node)),
        }
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
        Self {
            inner: Rc::new((score, Cell::new(score), node)),
        }
    }
}
impl<'a, T> Eq for ScoredNodeRef<'a, T> {}
impl<'a, T> PartialEq for ScoredNodeRef<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.2.eq(&other.inner.2)
    }
}
impl<'a, T> Hash for ScoredNodeRef<'a, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.2.id().hash(state)
    }
}

impl<TF, IDF, SOLVER> GdbrIdentifier<TF, IDF, SOLVER>
where
    TF: TfAlgorithm,
    IDF: IdfAlgorithm,
    SOLVER: Solver,
{
    fn is_possibly_gdbr_text_containing_element(node: &NodeRef<Node>) -> bool {
        match node.value() {
            Node::Element(element) => match element.name().parse::<HtmlTag>() {
                Ok(value) => match value {
                    HtmlTag::P
                    | HtmlTag::A
                    | HtmlTag::Div
                    | HtmlTag::Title
                    | HtmlTag::Dialog
                    | HtmlTag::Details => true,
                    otherwise => !matches!(
                        otherwise.category(),
                        HtmlTagCategory::Programming | HtmlTagCategory::StylesAndSemantics
                    ),
                },
                Err(err) => {
                    log::warn!(
                        "Was not able to identify {} as a tag! Treat is as true...",
                        err.0
                    );
                    true
                }
            },
            Node::Text(_) => {
                if let Some(parent) = node.parent() {
                    matches!(parent.value(), Node::Element(_))
                        && Self::is_possibly_gdbr_text_containing_element(&parent)
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    fn filter_fkt_without_type_filter<'a>(
        &self,
        node: NodeRef<'a, Node>,
    ) -> Option<(f64, NodeRef<'a, Node>)> {
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
            _ => None,
        }
    }

    fn filter_fkt<'a>(&self, node: NodeRef<'a, Node>) -> Option<(f64, NodeRef<'a, Node>)> {
        if Self::is_possibly_gdbr_text_containing_element(&node) {
            self.filter_fkt_without_type_filter(node)
        } else {
            None
        }
    }

    fn identify_gdbr_elements_in_html<'a>(
        &self,
        html: &'a Html,
    ) -> Option<Vec<Vec<ScoredNodeRef<'a, Node>>>> {
        let initial = html
            .tree
            .nodes()
            .into_iter()
            .filter(|value| {
                !value.has_children()
                    && if let Some(parent) = value.parent() {
                        if let Some(elem) = parent.value().as_element() {
                            if let Ok(tag) = elem.name().parse::<HtmlTag>() {
                                !matches!(
                                    tag.category(),
                                    HtmlTagCategory::Programming
                                        | HtmlTagCategory::StylesAndSemantics
                                )
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
            .filter_map(|node| {
                self.filter_fkt_without_type_filter(node)
                    .map(ScoredNodeRef::from)
            })
            .collect_vec();

        if initial.is_empty() {
            return None;
        }

        let mut visited: HashMap<_, _> = initial
            .iter()
            .map(|value| (value.node().id(), value.clone()))
            .collect();

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
                0 => {}
                1 => result_collection.push(Vec::from_iter(result)),
                _ => {
                    result_collection.push(Vec::from_iter(result));
                    continue;
                }
            }
            break Some(result_collection);
        }
    }

    fn get_most_probable<'a>(&self, html: &'a Html) -> Option<ScoredNodeRef<'a, Node>> {
        if let Some(gdbr_nodes) = self.identify_gdbr_elements_in_html(html) {
            let value = gdbr_nodes.into_iter().rev().next()?;
            self.filter_by.find_max_by(value, self.filter_threshold)
        } else {
            None
        }
    }

    /// Removes the gbr from the parsed html
    pub fn remove_gdbr(&self, html: &mut Html) {
        if let Some(found) = self.get_most_probable(&html) {
            let mut node = unsafe { html.tree.get_unchecked_mut(found.node().id()) };
            node.detach()
        }
    }

    #[cfg(test)]
    pub fn has_gbr(&self, html: &str) -> bool {
        let html = Html::parse_document(html);
        self.get_most_probable(&html).is_some()
    }
}

#[cfg(test)]
mod test {
    use crate::gdbr::identifier::{FilterMode, GdbrIdentifier};
    use crate::gdbr::scraper_ext::Text;
    use camino::Utf8PathBuf;
    use isolang::Language;
    use itertools::Itertools;
    use liblinear::parameter::serde::GenericParameters;
    use liblinear::solver::L2R_L2LOSS_SVR;
    use rust_stemmers::Algorithm;
    use scraper::{Html, Node};
    use std::io::Read;
    use std::ops::Deref;
    use svm::classifier::DocumentClassifier;
    use svm::config::DocumentClassifierConfig;
    use svm::{read_train_data, train, CsvProvider, CsvTrainModelEntry};
    use text_processing::configs::StopwordRegistryConfig;
    use text_processing::stopword_registry::{StopWordRegistry, StopWordRepository};
    use text_processing::tf_idf::{Idf, Tf};

    fn create_german_gdbr_svm() -> DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR> {
        let reg = StopwordRegistryConfig {
            registries: vec![StopWordRepository::IsoDefault],
        };
        let reg = StopWordRegistry::initialize(&reg).unwrap();

        let cfg: DocumentClassifierConfig = DocumentClassifierConfig::new(
            text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.tf,
            text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.idf,
            "data/gdbr/de/svm.csv".into(),
            Some("data/gdbr/de/tf_idf.txt".into()),
            true,
            true,
            Some(Algorithm::German),
            Some(GenericParameters {
                epsilon: Some(0.0003),
                p: Some(0.1),
                cost: Some(10.0),
                ..GenericParameters::default()
            }),
            5,
            5,
        );

        train::<_, _, L2R_L2LOSS_SVR>(&Language::Deu, &cfg, reg.get_or_load(&Language::Deu))
            .expect("The training failed!")
    }

    fn train_data() -> CsvProvider<CsvTrainModelEntry, impl Read + Sized> {
        read_train_data::<Idf>(Utf8PathBuf::from("data/gdbr/de/svm.csv".to_string())).unwrap()
    }

    #[test]
    fn test_might() {
        const DATA: &'static str = include_str!("../../testdata/samples/Amazon.html");

        let identifier =
            GdbrIdentifier::new(create_german_gdbr_svm(), 0.1, 0.5, FilterMode::OnMaxScore);

        let html = Html::parse_document(DATA);
        let gdbr_nodes = identifier.identify_gdbr_elements_in_html(&html).unwrap();

        for (i, v) in gdbr_nodes.into_iter().enumerate() {
            println!("Level: {i}");
            for value in v {
                match value.node().value() {
                    Node::Text(_) => {
                        println!("    Text")
                    }
                    Node::Element(value) => {
                        println!("    Element: {}", value.name())
                    }
                    _ => println!("    Unsupported Type"),
                }
                let mut content = match value.node().value() {
                    Node::Text(value) => value.deref().to_string(),
                    Node::Element(_) => Text::traverse(&value.node()).join(" "),
                    _ => {
                        println!(">> ERROR with node!");
                        continue;
                    }
                };
                let mut result = identifier.tokenize(&content).into_iter().join(", ");
                content.truncate(100);
                result.truncate(100);
                println!(
                    "    {} ({}) - {content} ({})\n",
                    value.score(),
                    value.max_score(),
                    result
                );
            }
        }

        println!("\n\n####\n\n");
    }

    #[test]
    fn test_with_traindata() {
        let identifier =
            GdbrIdentifier::new(create_german_gdbr_svm(), 0.1, 0.5, FilterMode::OnScore);
        for value in train_data() {
            let result = identifier.has_gbr(&value.text);
            if result != value.is_class {
                println!("{result} || {} :: {}", value.is_class, value.text);
            }
        }
    }
}
