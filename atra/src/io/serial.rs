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

use std::fmt::Display;
use std::sync::atomic::*;
use std::sync::Arc;

#[derive(Clone, Copy, strum::Display)]
pub enum SerialValue {
    #[strum(to_string = "{0}")] Byte(u8),
    #[strum(to_string = "{0}")] Short(u16),
    #[strum(to_string = "{0}")] Int(u32),
    #[strum(to_string = "{0}")] Long(u64),
}

impl Default for  SerialValue {
    fn default() -> Self {
        Self::Int(0)
    }
}

#[derive(Copy, Clone)]
pub enum SerialProviderKind {
    None,
    Byte,
    Short,
    Int,
    Long
}

#[derive(Debug, Clone)]
pub enum SerialProvider {
    NoSerial,
    Byte{state:AtomicU8SerialState},
    Short{state:AtomicU16SerialState},
    Int{state:AtomicU32SerialState},
    Long{state:AtomicU64SerialState},
}

impl Default for SerialProvider {
    fn default() -> Self {
        Self::Int {state: Default::default()}
    }
}


impl From<SerialProviderKind> for SerialProvider {
    fn from(value: SerialProviderKind) -> Self {
        Self::new(value)
    }
}


/// Provides a serial
impl SerialProvider {
    pub fn new(serial_kind: SerialProviderKind) -> Self {
        match serial_kind {
            SerialProviderKind::None => {Self::NoSerial}
            SerialProviderKind::Byte => {SerialProvider::Byte {state: Default::default()}}
            SerialProviderKind::Short => {SerialProvider::Short {state: Default::default()}}
            SerialProviderKind::Int => {SerialProvider::Int {state: Default::default()}}
            SerialProviderKind::Long => {SerialProvider::Long {state: Default::default()}}
        }
    }

    pub fn with_initial_state(initial_state: SerialValue) -> Self {
        match initial_state {
            SerialValue::Byte(value) => {
                SerialProvider::Byte {state: AtomicU8SerialState::with_init_value(value)}
            }
            SerialValue::Short(value) => {
                SerialProvider::Short {state: AtomicU16SerialState::with_init_value(value)}
            }
            SerialValue::Int(value) => {
                SerialProvider::Int {state: AtomicU32SerialState::with_init_value(value)}
            }
            SerialValue::Long(value) => {
                SerialProvider::Long {state: AtomicU64SerialState::with_init_value(value)}
            }
        }
    }

    pub fn kind(&self) -> SerialProviderKind {
        match self {
            SerialProvider::NoSerial => {SerialProviderKind::None}
            SerialProvider::Byte { .. } => {SerialProviderKind::Byte}
            SerialProvider::Short { .. } => {SerialProviderKind::Short}
            SerialProvider::Int { .. } => {SerialProviderKind::Int}
            SerialProvider::Long { .. } => {SerialProviderKind::Long}
        }
    }

    pub fn provide_serial(&self) -> Option<SerialValue> {
        match self {
            SerialProvider::NoSerial => {
                None
            }
            SerialProvider::Byte { state } => {
                Some(SerialValue::Byte(state.get_next_serial()))
            }
            SerialProvider::Short { state } => {
                Some(SerialValue::Short(state.get_next_serial()))
            }
            SerialProvider::Int { state } => {
                Some(SerialValue::Int(state.get_next_serial()))
            }
            SerialProvider::Long { state } => {
                Some(SerialValue::Long(state.get_next_serial()))
            }
        }
    }
}

macro_rules! create_normal_provider {
    ($($ty: ty: $t: ident),+) => {
        $(
            paste::paste! {
                #[derive(Debug)]
                pub struct [<$t SerialState>] {
                    state: Arc<$t>
                }

                impl [<$t SerialState>] {
                    fn new() -> Self {
                        Self {
                            state: Default::default()
                        }
                    }

                    fn with_init_value(value: $ty) -> Self {
                        Self {
                            state: Arc::new($t::new(value))
                        }
                    }

                    pub fn get_next_serial(&self) -> $ty {
                        self.state.fetch_add(1, Ordering::SeqCst)
                    }
                }
                impl Clone for [<$t SerialState>] {
                    fn clone(&self) -> Self {
                        Self{state: self.state.clone()}
                    }
                }

                impl Default for [<$t SerialState>] {
                    fn default() -> Self {
                        Self::new()
                    }
                }
            }
        )+
    };
}

create_normal_provider! {
    u8: AtomicU8, u16: AtomicU16, u32: AtomicU32, u64: AtomicU64
}