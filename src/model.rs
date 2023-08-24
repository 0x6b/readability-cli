use std::{fmt::Display, str::FromStr};

#[derive(Clone)]
pub enum Model {
    Gpt4,
    Gpt4_32k,
    Gpt35Turbo,
}

impl FromStr for Model {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gpt-4" => Ok(Self::Gpt4),
            "4" => Ok(Self::Gpt4), // alias
            "gpt-4-32k" => Ok(Self::Gpt4_32k),
            "32k" => Ok(Self::Gpt4_32k), // alias
            "gpt-3.5-turbo" => Ok(Self::Gpt35Turbo),
            "35t" => Ok(Self::Gpt35Turbo), // alias
            _ => Ok(Self::Gpt35Turbo),
        }
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Gpt4 => "gpt-4",
            Self::Gpt4_32k => "gpt-4-32k",
            Self::Gpt35Turbo => "gpt-3.5-turbo",
        };
        write!(f, "{}", s)
    }
}
