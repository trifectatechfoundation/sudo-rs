use super::SudoDefault;

pub(super) trait ToSudoDefault {
    fn to_sudodefault(self) -> SudoDefault;
}

macro_rules! add_conversion {
    ($ctor:ident, $type:ty) => {
        impl ToSudoDefault for $type {
            fn to_sudodefault(self) -> SudoDefault {
                $crate::defaults::SudoDefault::$ctor(self.into())
            }
        }
    };

    ($ctor:ident, $type:ty, negatable$(, $vetting_function:expr)?) => {
        impl ToSudoDefault for $type {
            #[allow(clippy::from_str_radix_10)]
            fn to_sudodefault(self) -> SudoDefault {
                $crate::defaults::SudoDefault::$ctor(OptTuple {
                    default: self.into(),
                    negated: None,
                }$(, $vetting_function)?)
            }
        }

        impl ToSudoDefault for ($type, $type) {
            #[allow(clippy::from_str_radix_10)]
            fn to_sudodefault(self) -> SudoDefault {
                let (value, neg) = self;
                $crate::defaults::SudoDefault::$ctor(OptTuple {
                    default: value.into(),
                    negated: Some(neg.into()),
                }$(, $vetting_function)?)
            }
        }
    };
}

macro_rules! sliceify {
    ([$($value:tt),*]) => {
        &[$($value),*][..]
    };
    ($value:tt) => {
        ($value)
    };
}

macro_rules! tupleify {
    ($fst:expr, $snd:expr) => {
        ($fst, $snd)
    };
    ($value:tt) => {
        $value
    };
}

macro_rules! optional {
    () => {
        |x| x
    };
    ($block: block) => {
        $block
    };
}

macro_rules! type_of {
    ($id:ident, true) => { bool };
    ($id:ident, false) => { bool };
    ($id:ident, [ $($value: expr),* ]) => { std::collections::HashSet<String> };
    ($id:ident, $(=int $check: expr;)+ $_: expr) => { i64 };
    ($id:ident, $(=enum $k: ident;)+ $_: ident) => { $crate::defaults::enums::$id };
    ($id:ident, $_: expr) => { Option<Box<str>> };
}

macro_rules! value_of {
    ($id:ident, true) => { true };
    ($id:ident, false) => { false };
    ($id:ident, [ $($value: expr),* ]) => { [$($value),*].into_iter().map(|s: &str| s.to_string()).collect::<std::collections::HashSet<_>>() };
    ($id:ident, $(=int $check: expr;)+ $value: expr) => { $value };
    ($id:ident, $(=enum $k: ident;)+ $value: ident) => { $crate::defaults::enums::$id::$value };
    ($id:ident, None) => { None };
    ($id:ident, $value: expr) => { Some($value.into()) };
    ($id:ident, $($_: tt)*) => { ifdef![] };
}

macro_rules! negate_of {
    ($id:ident, true) => {
        false
    };
    ($id:ident, false) => {
        false
    };
    ($id:ident, [ $($value: expr),* ]) => {
        std::collections::HashSet::new()
    };
    ($id:ident, $(=int $check: expr;)+ $value: expr) => {
        ifdef![]
    };
    ($id:ident, $(=enum $k: ident;)+ $value: ident) => {
        ifdef![]
    };
    ($id:ident, None) => {
        ifdef![]
    };
    ($id:ident, $value: expr) => {
        ifdef![]
    };
    ($id:ident, $($_: tt)*) => {
        ifdef![]
    };
}

macro_rules! modifier_of {
    ($id:ident, true) => {
        $crate::defaults::SettingKind::Flag(Box::new(move |obj: &mut Settings| obj.$id = true) as _)
    };
    ($id:ident, false) => {
        $crate::defaults::SettingKind::Flag(Box::new(move |obj: &mut Settings| obj.$id = true) as _)
    };
    ($id:ident, [ $($value: expr),* ]) => {
        $crate::defaults::SettingKind::List(
            |mode, list|
                Some(Box::new(move |obj: &mut Settings|
                                       match mode {
                                          ListMode::Set => obj.$id = list.into_iter().collect(),
                                          ListMode::Add => obj.$id.extend(list),
                                          ListMode::Del => for key in [$($value),*] {
                                                               obj.$id.remove(key);
                                                           },
                                       }) as _)
        )
    };
    ($id:ident, =int $first:literal ..= $last: literal $(@ $radix: literal)?; $value: expr) => {
        #[allow(clippy::from_str_radix_10)]
        $crate::defaults::SettingKind::Integer(
            |text| i64::from_str_radix(text, 10$(*0 + $radix)?)
                            .ok()
                            .filter(|val| ($first ..= $last)
                            .contains(val))
                            .map(|i| Box::new(move |obj: &mut Settings| obj.$id = i) as _)
        )
    };
    ($id:ident, =int $fn: expr; $value: expr) => {
        $crate::defaults::SettingKind::Integer(
            |text| $fn(&text).map(|i| Box::new(move |obj: &mut Settings| obj.$id = i) as _)
        )
    };
    ($id:ident, $(=int $check: expr;)+ $value: expr) => { compile_error!("bla") };
    ($id:ident, $(=enum $key: ident;)+ $value: ident) => {
        $crate::defaults::SettingKind::Text(|key| match key {
            $(
            stringify!($key) => { Some(Box::new(move |obj: &mut Settings| obj.$id = $crate::defaults::enums::$id::$key) as _) },
            )*
            _ => None,
        })
    };
    ($id:ident, None) => {
        $crate::defaults::SettingKind::Text(
            |text| {
                let text = text.into();
                Some(Box::new(move |obj: &mut Settings| obj.$id = Some(text)) as _)
            }
        )
    };
    ($id:ident, $value: expr) => {
        $crate::defaults::SettingKind::Text(
            |text| {
                let text = text.into();
                Some(Box::new(move |obj: &mut Settings| obj.$id = Some(text)) as _)
            }
        )
    };
}

// this macro allows us to help the compiler generate more efficient code in 'fn negate'
// and enables the way 'fn set' is made
macro_rules! ifdef {
    (; $then: expr; $else: expr) => {
        $else
    };
    ($($_: expr)+; $then: expr; $else: expr) => {
        $then
    };

    () => {
        return None
    };
    ($body: expr) => {
        $body
    };
}

macro_rules! defaults {
    ($($name:ident = $value:tt $((!= $negate:tt))? $([$($key:ident),*])? $([$first:literal ..= $last:literal$(; radix: $radix: expr)?])? $({$fn: expr})?)*) => {
        #[allow(non_camel_case_types)]
        mod enums {
            $($(
                #[derive(Debug)]
                #[cfg_attr(test, derive(PartialEq, Eq))]
                pub enum $name { $($key),* }
            )?)*
        }

        pub struct Settings {
            $(pub $name: type_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value)),*
        }

        impl Default for Settings {
            #[allow(unused_parens)]
            fn default() -> Self {
                Self {
                    $($name: value_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value)),*
                }
            }
        }

        #[allow(clippy::diverging_sub_expression)]
        #[allow(unreachable_code)]
        pub fn negate(name: &str) -> Option<SettingsModifier> {
            match name {
                $(
                stringify!($name) if ifdef!($($negate)?; true; false) || matches!(stringify!($value), "true" | "false") || stringify!($value).starts_with('[') => {
                    let _value = ifdef!($($negate)?;
                        value_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $($negate)?);
                        negate_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value)
                    );
                    Some(Box::new(move |obj: &mut Settings| obj.$name = _value))
                },
                )*
                _ => None
            }
        }

        pub fn set(name: &str) -> Option<SettingKind> {
            match name {
            $(
                stringify!($name) => Some(modifier_of!($name, $(=int $fn;)?$(=int $first ..= $last $(@ $radix)?;)?$($(=enum $key;)*)? $value)),
            )*
                _ => None,
            }
        }
    };
}

macro_rules! old_defaults {
    ($($name:ident = $value:tt $((!= $negate:tt))? $([$($key:ident),*])? $([$first:literal ..= $last:literal$(; radix: $radix: expr)?])? $({$fn: expr})?)*) => {
        pub const ALL_PARAMS: &'static [&'static str] = &[
            $(stringify!($name)),*
        ];

        add_conversion!(Flag, bool);
        add_conversion!(Integer, i64, negatable, |text| i64::from_str_radix(text, 10).ok());
        add_conversion!(Text, &'static str, negatable);
        add_conversion!(Text, Option<&'static str>, negatable);
        add_conversion!(List, &'static [&'static str]);
        add_conversion!(Enum, StrEnum<'static>, negatable);

        // because of the nature of radix and ranges, 'let mut result' is not always necessary, and
        // a radix of 10 can also not always be avoided (and for uniformity, I would also not avoid this
        // if this was hand-written code.
        #[allow(unused_mut)]
        #[allow(clippy::from_str_radix_10)]
        pub fn sudo_default(var: &str) -> Option<SudoDefault> {
            Some(
                match var {
                    $(stringify!($name) => {
                          let restrict = optional![$({
                              let keys = &[$(stringify!($key)),*];
                              |key: &'static str| StrEnum::new(key, keys).unwrap_or_else(|| unreachable!())
                          })?];
                          let datum = restrict(sliceify!($value));
                          let mut result = tupleify!(datum$(, restrict($negate))?).to_sudodefault();
                          $(
                              if let SudoDefault::Integer(_, ref mut checker) = &mut result {
                                  *checker = |text| i64::from_str_radix(text, 10$(*0 + $radix)?).ok().filter(|val| ($first ..= $last).contains(val));
                              }
                          )?
                          $(
                              if let SudoDefault::Integer(_, ref mut checker) = &mut result {
                                  *checker = $fn
                              }
                          )?
                          result
                    },
                    )*
                    _ => return None
                }
            )
        }
    };
}

pub(super) use add_conversion;
pub(super) use defaults;
pub(super) use ifdef;
pub(super) use modifier_of;
pub(super) use negate_of;
pub(super) use old_defaults;
pub(super) use optional;
pub(super) use sliceify;
pub(super) use tupleify;
pub(super) use type_of;
pub(super) use value_of;
