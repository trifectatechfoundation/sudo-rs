macro_rules! add_from {
    ($ctor:ident, $type:ty) => {
        #[allow(non_local_definitions)]
        impl From<$type> for $crate::defaults::SudoDefault {
            fn from(value: $type) -> Self {
                $crate::defaults::SudoDefault::$ctor(value.into())
            }
        }
    };

    ($ctor:ident, $type:ty, negatable$(, $vetting_function:expr)?) => {
        #[allow(non_local_definitions)]
        impl From<$type> for $crate::defaults::SudoDefault {
            fn from(value: $type) -> Self {
                $crate::defaults::SudoDefault::$ctor(OptTuple {
                    default: value.into(),
                    negated: None,
                }$(, $vetting_function)?)
            }
        }

        #[allow(non_local_definitions)]
        impl From<($type, $type)> for $crate::defaults::SudoDefault {
            fn from((value, neg): ($type, $type)) -> Self {
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

macro_rules! defaults {
    ($($name:ident = $value:tt $((!= $negate:tt))? $([$($key:ident),*])? $([$first:literal ..= $last:literal$(; radix: $radix: expr)?])? $({$fn: expr})?)*) => {
        pub const ALL_PARAMS: &'static [&'static str] = &[
            $(stringify!($name)),*
        ];

        // because of the nature of radix and ranges, 'let mut result' is not always necessary, and
        // a radix of 10 can also not always be avoided (and for uniformity, I would also not avoid this
        // if this was hand-written code.
        #[allow(unused_mut)]
        #[allow(clippy::from_str_radix_10)]
        pub fn sudo_default(var: &str) -> Option<SudoDefault> {
            add_from!(Flag, bool);
            add_from!(Integer, i64, negatable, |text| i64::from_str_radix(text, 10).ok());
            add_from!(Text, &'static str, negatable);
            add_from!(Text, Option<&'static str>, negatable);
            add_from!(List, &'static [&'static str]);
            add_from!(Enum, StrEnum<'static>, negatable);

            Some(
                match var {
                    $(stringify!($name) => {
                          let restrict = optional![$({
                              let keys = &[$(stringify!($key)),*];
                              |key: &'static str| StrEnum::new(key, keys).unwrap_or_else(|| unreachable!())
                          })?];
                          let datum = restrict(sliceify!($value));
                          let mut result = tupleify!(datum$(, restrict($negate))?).into();
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

pub(super) use add_from;
pub(super) use defaults;
pub(super) use optional;
pub(super) use sliceify;
pub(super) use tupleify;
