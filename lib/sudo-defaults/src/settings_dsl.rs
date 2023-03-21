macro_rules! add_from {
    ($ctor:ident, $type:ty) => {
        impl From<$type> for $crate::SudoDefault {
            fn from(value: $type) -> Self {
                $crate::SudoDefault::$ctor(value)
            }
        }
    };

    ($ctor:ident, $type:ty, negatable$(, $vetting_function:expr)?) => {
        impl From<$type> for $crate::SudoDefault {
            fn from(value: $type) -> Self {
                $crate::SudoDefault::$ctor(OptTuple {
                    default: value,
                    negated: None,
                }$(, $vetting_function)?)
            }
        }

        impl From<($type, $type)> for $crate::SudoDefault {
            fn from((value, neg): ($type, $type)) -> Self {
                $crate::SudoDefault::$ctor(OptTuple {
                    default: value,
                    negated: Some(neg),
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
    ($($name:ident = $value:tt $((!= $negate:tt))? $([$($key:ident),*])? $([#$radix:expr, $range:expr])?)*) => {
        pub const ALL_PARAMS: &'static [&'static str] = &[
            $(stringify!($name)),*
        ];

        #[allow(unused_mut)]
        pub fn sudo_default(var: &str) -> Option<SudoDefault> {
            add_from!(Flag, bool);
            add_from!(Integer, i128, negatable, |text| i128::from_str_radix(text, 10).ok());
            add_from!(Text, &'static str, negatable);
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
                                  *checker = |text| i128::from_str_radix(text, $radix).ok().filter(|val| $range.contains(val));
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
