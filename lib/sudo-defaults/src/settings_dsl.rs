macro_rules! add_from {
    ($ctor:ident, $type:ty) => {
        impl From<$type> for $crate::SudoDefault {
            fn from(value: $type) -> Self {
                $crate::SudoDefault::$ctor(value)
            }
        }
    };

    ($ctor:ident, $type:ty, negatable) => {
        impl From<$type> for $crate::SudoDefault {
            fn from(value: $type) -> Self {
                $crate::SudoDefault::$ctor(OptTuple {
                    default: value,
                    negated: None,
                })
            }
        }

        impl From<($type, $type)> for $crate::SudoDefault {
            fn from((value, neg): ($type, $type)) -> Self {
                $crate::SudoDefault::$ctor(OptTuple {
                    default: value,
                    negated: Some(neg),
                })
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
    ($($name:ident = $value:tt $((!= $negate:tt))? $([$($key:ident),*])?)*) => {
        pub fn sudo_default(var: &str) -> Option<SudoDefault> {
            add_from!(Flag, bool);
            add_from!(Integer, i128, negatable);
            add_from!(Text, &'static str, negatable);
            add_from!(List, &'static [&'static str]);

            impl<T: Into<$crate::SudoDefault>> From<(T, &'static[&'static str])> for $crate::SudoDefault {
                fn from((obj,list): (T, &'static[&'static str])) -> Self {
                    let $crate::SudoDefault::Text(value) = obj.into() else { unreachable!() };
                    $crate::SudoDefault::Enum(value, list)
                }
            }

            Some(
                match var {
                    $(stringify!($name) => {
                          $(let keys = &[$(stringify!($key)),*];)?
                          let restrict = optional![$({
                              let _ = || &[$(stringify!($key)),*];
                              |key: &'static str| -> &'static str {
                                  let elem = keys.iter().find(|&k| k == &key).unwrap_or_else(|| unreachable!());
                                  elem
                              }
                          })?];
                          let embellish = optional![$({
                              let _ = || &[$(stringify!($key)),*];
                              |def| match def {
                                  $crate::SudoDefault::Text(value) => $crate::SudoDefault::Enum(value, keys),
                                  x => x,
                              }
                          })?];
                          let datum = restrict(sliceify!($value));
                          embellish(tupleify!(datum$(, restrict($negate))?).into())
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
