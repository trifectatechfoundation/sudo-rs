use core::{fmt, ops};

/// `String` but guaranteed to not be empty
#[derive(PartialEq)]
pub struct NonEmptyString {
    inner: String,
}

#[cfg(test)]
impl From<&'_ str> for NonEmptyString {
    fn from(value: &str) -> Self {
        Self::new(value.to_string()).unwrap()
    }
}

#[cfg(test)]
impl PartialEq<&'_ NonEmptyString> for str {
    fn eq(&self, other: &&NonEmptyString) -> bool {
        self == other.inner
    }
}

impl fmt::Debug for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl ops::Deref for NonEmptyString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl NonEmptyString {
    pub fn new(string: String) -> Option<Self> {
        if string.is_empty() {
            None
        } else {
            Some(Self { inner: string })
        }
    }
}
