macro_rules! storage_of {
    ($id:ident, true) => { bool };
    ($id:ident, false) => { bool };
    ($id:ident, [ $($value: expr),* ]) => { std::collections::HashSet<String> };
    ($id:ident, $(=int $check: expr;)+ $_: expr) => { i64 };
    ($id:ident, $(=enum $k: ident;)+ $_: ident) => { $crate::defaults::enums::$id };
    ($id:ident, $_: expr) => { Option<Box<str>> };
}

macro_rules! referent_of {
    ($id:ident, true) => { bool };
    ($id:ident, false) => { bool };
    ($id:ident, [ $($value: expr),* ]) => { &std::collections::HashSet<String> };
    ($id:ident, $(=int $check: expr;)+ $_: expr) => { i64 };
    ($id:ident, $(=enum $k: ident;)+ $_: ident) => { $crate::defaults::enums::$id };
    ($id:ident, $_: expr) => { Option<&str> };
}

macro_rules! initializer_of {
    ($id:ident, true) => { true };
    ($id:ident, false) => { false };
    ($id:ident, [ $($value: expr),* ]) => { [$($value),*].into_iter().map(|s: &str| s.to_string()).collect::<std::collections::HashSet<_>>() };
    ($id:ident, $(=int $check: expr;)+ $value: expr) => { $value };
    ($id:ident, $(=enum $k: ident;)+ $value: ident) => { $crate::defaults::enums::$id::$value };
    ($id:ident, None) => { None };
    ($id:ident, $value: expr) => { Some($value.into()) };
    ($id:ident, $($_: tt)*) => { ifdef![] };
}

macro_rules! negator_of {
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

macro_rules! result_of {
    ($id:expr, true) => {
        $id
    };
    ($id:expr, false) => {
        $id
    };
    ($id:expr, [ $($value: expr),* ]) => {
        &$id
    };
    ($id:expr, $(=value $k: expr;)+ $_: expr) => {
        $id
    };
    ($id:expr, $_: expr) => {
        $id.as_deref()
    };
}

macro_rules! modifier_of {
    ($id:ident, true) => {
        $crate::defaults::SettingKind::Flag(Box::new(move |obj: &mut Settings| obj.$id = true))
    };
    ($id:ident, false) => {
        $crate::defaults::SettingKind::Flag(Box::new(move |obj: &mut Settings| obj.$id = true))
    };
    ($id:ident, [ $($value: expr),* ]) => {
        $crate::defaults::SettingKind::List(
            |mode, list|
                Box::new(move |obj: &mut Settings|
                                   match mode {
                                      ListMode::Set => obj.$id = list.into_iter().collect(),
                                      ListMode::Add => obj.$id.extend(list),
                                      ListMode::Del => for key in list {
                                                           obj.$id.remove(&key);
                                                       },
                                   })
        )
    };
    ($id:ident, =int $first:literal ..= $last: literal $(@ $radix: literal)?; $value: expr) => {
        #[allow(clippy::from_str_radix_10)]
        $crate::defaults::SettingKind::Integer(
            |text| i64::from_str_radix(text, 10$(*0 + $radix)?)
                            .ok()
                            .filter(|val| ($first ..= $last)
                            .contains(val))
                            .map(|i| Box::new(move |obj: &mut Settings| obj.$id = i) as Box<dyn FnOnce(&mut Settings)>)
        )
    };
    ($id:ident, =int $fn: expr; $value: expr) => {
        $crate::defaults::SettingKind::Integer(
            |text| $fn(&text).map(|i| Box::new(move |obj: &mut Settings| obj.$id = i) as Box<dyn FnOnce(&mut Settings)>)
        )
    };
    ($id:ident, $(=int $check: expr;)+ $value: expr) => { compile_error!("bla") };
    ($id:ident, $(=enum $key: ident;)+ $value: ident) => {
        $crate::defaults::SettingKind::Text(|key| match key {
            $(
            stringify!($key) => { Some(Box::new(move |obj: &mut Settings| obj.$id = $crate::defaults::enums::$id::$key)) },
            )*
            _ => None,
        })
    };
    ($id:ident, None) => {
        $crate::defaults::SettingKind::Text(
            |text| {
                let text = text.into();
                Some(Box::new(move |obj: &mut Settings| obj.$id = Some(text)))
            }
        )
    };
    ($id:ident, $value: expr) => {
        $crate::defaults::SettingKind::Text(
            |text| {
                let text = text.into();
                Some(Box::new(move |obj: &mut Settings| obj.$id = Some(text)))
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

macro_rules! emit {
    (ignored; $($def: tt)*) => { };
    ( ; $($def: tt)*) => { $($def)* };
}

macro_rules! defaults {
    ($($name:ident = $value:tt $((!= $negate:tt))? $([$($key:ident),*])? $([$first:literal ..= $last:literal$(; radix: $radix: expr)?])? $({$fn: expr})? $(#$attribute:ident)?)*) => {
        #[allow(non_camel_case_types)]
        mod enums {
            $($(
                #[derive(Clone,Copy,Debug)]
                #[cfg_attr(test, derive(PartialEq, Eq))]
                pub enum $name { $($key),* }
            )?)*
        }

        #[derive(Clone)]
        pub struct Settings {
            $($name: storage_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value)),*
        }

        // we add setters to make sure the settings-object is read only, and to generate 'unused variable' warnings
        impl Settings {
            $(
            emit! { $($attribute)?;
                pub fn $name(&self) -> referent_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value) {
                    result_of!(self.$name, $(=value $fn;)?$(=value $first;)?$($(=value $key;)*)? $value)
                }
            }
            )*
        }

        impl Default for Settings {
            #[allow(unused_parens)]
            fn default() -> Self {
                Self {
                    $($name: initializer_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value)),*
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
                        initializer_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $($negate)?);
                        negator_of!($name, $(=int $fn;)?$(=int $first;)?$($(=enum $key;)*)? $value)
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

pub(super) use defaults;
pub(super) use emit;
pub(super) use ifdef;
pub(super) use initializer_of;
pub(super) use modifier_of;
pub(super) use negator_of;
pub(super) use referent_of;
pub(super) use result_of;
pub(super) use storage_of;
