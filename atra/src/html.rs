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

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use strum::{Display, EnumIs};
use thiserror::Error;

macro_rules! html_tags {
    ($($name: ident = $value: literal;)+) => {

        #[derive(Serialize, Deserialize, Debug, Display)]
        pub enum HtmlTag {
            $($name),+
        }

        impl HtmlTag {
            pub fn tag(&self) -> &'static str {
                match self {
                    $(Self::$name => $value),+
                }
            }
        }

        impl FromStr for HtmlTag {
            type Err = NotAHtmlTag;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($value => Ok(Self::$name),)+
                    unknown => Err(NotAHtmlTag(unknown.to_string()))
                }
            }
        }
    };
}

macro_rules! hmtl_tags_categories {
    ($($cat: ident: $($tag: ident),+;)+) => {
        #[derive(Serialize, Deserialize, Debug, EnumIs, Display)]
        pub enum HtmlTagCategory {
            $($cat),+
        }

        impl HtmlTagCategory {
            pub fn tags(&self) -> &'static [HtmlTag] {
                match self {
                    $(Self::$cat => &[$(HtmlTag::$tag),+]),+
                }
            }
        }

        impl HtmlTag {
            pub fn category(&self) -> HtmlTagCategory {
                match self {
                    $($(Self::$tag)|+ => HtmlTagCategory::$cat),+
                }
            }
        }
    };
}

#[derive(Debug, Error)]
#[error("The value \"{0}\" is not a tag!")]
pub struct NotAHtmlTag(pub String);

html_tags! {
    A = "a";
    Abbr = "abbr";
    Acronym = "acronym";
    Address = "address";
    Applet = "applet";
    Area = "area";
    Article = "article";
    Aside = "aside";
    Audio = "audio";
    B = "b";
    Base = "base";
    Basefont = "basefont";
    Bdi = "bdi";
    Bdo = "bdo";
    Big = "big";
    Blockquote = "blockquote";
    Body = "body";
    Br = "br";
    Button = "button";
    Canvas = "canvas";
    Caption = "caption";
    Center = "center";
    Cite = "cite";
    Code = "code";
    Col = "col";
    Colgroup = "colgroup";
    Data = "data";
    Datalist = "datalist";
    Dd = "dd";
    Del = "del";
    Details = "details";
    Dfn = "dfn";
    Dialog = "dialog";
    Dir = "dir";
    Div = "div";
    Dl = "dl";
    Dt = "dt";
    Em = "em";
    Embed = "embed";
    Fieldset = "fieldset";
    Figcaption = "figcaption";
    Figure = "figure";
    Font = "font";
    Footer = "footer";
    Form = "form";
    Frame = "frame";
    Frameset = "frameset";
    H1 = "h1";
    H2 = "h2";
    H3 = "h3";
    H4 = "h4";
    H5 = "h5";
    H6 = "h6";
    Head = "head";
    Header = "header";
    Hgroup = "hgroup";
    Hr = "hr";
    Html = "html";
    I = "i";
    Iframe = "iframe";
    Img = "img";
    Input = "input";
    Ins = "ins";
    Kbd = "kbd";
    Label = "label";
    Legend = "legend";
    Li = "li";
    Link = "link";
    Main = "main";
    Map = "map";
    Mark = "mark";
    Menu = "menu";
    Meta = "meta";
    Meter = "meter";
    Nav = "nav";
    Noframes = "noframes";
    Noscript = "noscript";
    Object = "object";
    Ol = "ol";
    Optgroup = "optgroup";
    Option = "option";
    Output = "output";
    P = "p";
    Param = "param";
    Picture = "picture";
    Pre = "pre";
    Progress = "progress";
    Q = "q";
    Rp = "rp";
    Rt = "rt";
    Ruby = "ruby";
    S = "s";
    Samp = "samp";
    Script = "script";
    Search = "search";
    Section = "section";
    Select = "select";
    Small = "small";
    Source = "source";
    Span = "span";
    Strike = "strike";
    Strong = "strong";
    Style = "style";
    Sub = "sub";
    Summary = "summary";
    Sup = "sup";
    Svg = "svg";
    Table = "table";
    Tbody = "tbody";
    Td = "td";
    Template = "template";
    Textarea = "textarea";
    Tfoot = "tfoot";
    Th = "th";
    Thead = "thead";
    Time = "time";
    Title = "title";
    Tr = "tr";
    Track = "track";
    Tt = "tt";
    U = "u";
    Ul = "ul";
    Var = "var";
    Video = "video";
    Wbr = "wbr";
}

hmtl_tags_categories!(
    Basic: Html, Head, Title, Body, H1, H2, H3, H4, H5, H6, P, Br, Hr;
    Formatting: Acronym, Abbr, Address, B, Bdi, Bdo, Big, Blockquote, Center, Cite, Code, Del, Dfn, Em, Font, I, Ins, Kbd, Mark, Meter, Pre, Progress, Q, Rp, Rt, Ruby, S, Samp, Small, Strike, Strong, Sub, Sup, Template, Time, Tt, U, Var, Wbr;
    FormsAndImput: Form, Input, Textarea, Button, Select, Optgroup, Option, Label, Fieldset, Legend, Datalist, Output;
    Frames: Frame, Frameset, Noframes, Iframe;
    Images: Img, Map, Area, Canvas, Figcaption, Figure, Picture, Svg;
    AudioOrVideo: Audio, Source, Track, Video;
    Links: A, Link, Nav;
    Lists: Menu, Ul, Ol, Li, Dir, Dl, Dt, Dd;
    Tables: Table, Caption, Th, Tr, Td, Thead, Tbody, Tfoot, Col, Colgroup;
    StylesAndSemantics: Style, Div, Span, Header, Hgroup, Footer, Main, Section, Search, Article, Aside, Details, Dialog, Summary, Data;
    MetaInfo: Meta, Base, Basefont;
    Programming: Script, Noscript, Applet, Embed, Object, Param;
);
