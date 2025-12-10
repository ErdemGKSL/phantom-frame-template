pub enum Environment {
    Development,
    Production,
}

impl Into<&'static str> for Environment {
    fn into(self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Production => "production",
        }
    }
}

impl Into<Environment> for &str {
    fn into(self) -> Environment {
        match self {
            "production" => Environment::Production,
            _ => Environment::Development,
        }
    }
}

pub fn get_enviroment() -> Environment {
    #[cfg(debug_assertions)]
    return Environment::Development;
    #[cfg(not(debug_assertions))]
    return Environment::Production;
}