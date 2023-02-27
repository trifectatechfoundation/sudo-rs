use super::SudoDefault;

macro_rules! add_from {
    ($ctor:ident, $type:ty) => {
        impl From<$type> for SudoDefault {
            fn from(value: $type) -> Self {
                SudoDefault::$ctor(value)
            }
        }
    };

    ($ctor:ident, $type:ty, negatable) => {
        impl From<$type> for SudoDefault {
            fn from(value: $type) -> Self {
                SudoDefault::$ctor((value, None))
            }
        }

        impl From<($type, $type)> for SudoDefault {
            fn from((value, neg): ($type, $type)) -> Self {
                SudoDefault::$ctor((value, Some(neg)))
            }
        }
    };
}

add_from!(Flag, bool);
add_from!(Integer, usize, negatable);
add_from!(Text, &'static str, negatable);
add_from!(List, &'static [&'static str]);

macro_rules! sliceify {
    ([$($value:tt),*]) => {
        &[$($value),*][..]
    };
    ($value:tt) => { $value };
}

macro_rules! tupleify {
    ($fst:expr, $snd:expr) => {
        ($fst, $snd)
    };
    ($value:tt) => {
        $value
    };
}

macro_rules! defaults {
    ($($name:ident = $value:tt $((!= $negate:tt))?)*) => {
        pub fn sudo_default(var: &str) -> Option<SudoDefault> {
            Some(
                match var {
                    $(stringify!($name) => {
                          let datum = sliceify!($value);
                          tupleify!(datum$(, $negate)?).into()
                    },
                    )*
                    _ => return None
                }
            )
        }
    };
}

pub(super) use defaults;
pub(super) use sliceify;
pub(super) use tupleify;
