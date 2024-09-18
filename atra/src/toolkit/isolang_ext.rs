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

use isolang::Language;
use std::convert::Infallible;
use std::error::Error as StdError;
use whatlang::Lang;

pub trait ToIsoLang {
    fn to_isolang(self) -> Language;
}

pub trait TryToIsoLang {
    type Error: StdError;

    fn try_to_isolang(self) -> Result<Language, Self::Error>;
}

impl<T> TryToIsoLang for T
where
    T: ToIsoLang,
{
    type Error = Infallible;

    #[inline(always)]
    fn try_to_isolang(self) -> Result<Language, Self::Error> {
        Ok(self.to_isolang())
    }
}

impl ToIsoLang for Lang {
    fn to_isolang(self) -> Language {
        match self {
            Lang::Epo => Language::Epo,
            Lang::Eng => Language::Eng,
            Lang::Rus => Language::Rus,
            Lang::Cmn => Language::Cmn,
            Lang::Spa => Language::Spa,
            Lang::Por => Language::Por,
            Lang::Ita => Language::Ita,
            Lang::Ben => Language::Ben,
            Lang::Fra => Language::Fra,
            Lang::Deu => Language::Deu,
            Lang::Ukr => Language::Ukr,
            Lang::Kat => Language::Kat,
            Lang::Ara => Language::Ara,
            Lang::Hin => Language::Hin,
            Lang::Jpn => Language::Jpn,
            Lang::Heb => Language::Heb,
            Lang::Yid => Language::Yid,
            Lang::Pol => Language::Pol,
            Lang::Amh => Language::Amh,
            Lang::Jav => Language::Jav,
            Lang::Kor => Language::Kor,
            Lang::Nob => Language::Nob,
            Lang::Dan => Language::Dan,
            Lang::Swe => Language::Swe,
            Lang::Fin => Language::Fin,
            Lang::Tur => Language::Tur,
            Lang::Nld => Language::Nld,
            Lang::Hun => Language::Hun,
            Lang::Ces => Language::Ces,
            Lang::Ell => Language::Ell,
            Lang::Bul => Language::Bul,
            Lang::Bel => Language::Bel,
            Lang::Mar => Language::Mar,
            Lang::Kan => Language::Kan,
            Lang::Ron => Language::Ron,
            Lang::Slv => Language::Slv,
            Lang::Hrv => Language::Hrv,
            Lang::Srp => Language::Srp,
            Lang::Mkd => Language::Mkd,
            Lang::Lit => Language::Lit,
            Lang::Lav => Language::Lav,
            Lang::Est => Language::Est,
            Lang::Tam => Language::Tam,
            Lang::Vie => Language::Vie,
            Lang::Urd => Language::Urd,
            Lang::Tha => Language::Tha,
            Lang::Guj => Language::Guj,
            Lang::Uzb => Language::Uzb,
            Lang::Pan => Language::Pan,
            Lang::Aze => Language::Aze,
            Lang::Ind => Language::Ind,
            Lang::Tel => Language::Tel,
            Lang::Pes => Language::Pes,
            Lang::Mal => Language::Mal,
            Lang::Ori => Language::Ori,
            Lang::Mya => Language::Mya,
            Lang::Nep => Language::Nep,
            Lang::Sin => Language::Sin,
            Lang::Khm => Language::Khm,
            Lang::Tuk => Language::Tuk,
            Lang::Aka => Language::Aka,
            Lang::Zul => Language::Zul,
            Lang::Sna => Language::Sna,
            Lang::Afr => Language::Afr,
            Lang::Lat => Language::Lat,
            Lang::Slk => Language::Slk,
            Lang::Cat => Language::Cat,
            Lang::Tgl => Language::Tgl,
            Lang::Hye => Language::Hye,
        }
    }
}
