/// This is a light-weight "enum"; it restricts the allowed values, but doesn't offer as much
/// compile-time guarantees as an actual Enum. On the other hand this allows for a bit more
/// flexibility.

#[derive(Debug, Clone)]
pub struct StrEnum<'a> {
    pub(super) value: &'a str,
    pub possible_values: &'a [&'a str],
}

impl<'a> StrEnum<'a> {
    pub fn new(choice: &str, possible_values: &'a [&'a str]) -> Option<Self> {
        Some(StrEnum {
            value: possible_values.iter().find(|&key| *key == choice)?,
            possible_values,
        })
    }

    pub fn alt(self, choice: &str) -> Option<Self> {
        Self::new(choice, self.possible_values)
    }

    pub fn get(&self) -> &'a str {
        self.value
    }
}

impl<'a> std::ops::Deref for StrEnum<'a> {
    type Target = str;
    fn deref(&self) -> &str {
        self.get()
    }
}
