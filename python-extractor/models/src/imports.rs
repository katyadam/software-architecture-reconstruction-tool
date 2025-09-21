// Differentiating between original and used naming, due to aliasing.
#[derive(Debug, PartialEq, Eq)]
pub struct Import {
    pub orig_module: String,
    pub orig_name: String,
    pub module_alias: String,
    pub name_alias: String,
    pub codeword: String,
}
impl Import {
    pub fn from_parts(
        orig_module: &str,
        orig_name: &str,
        module_alias: &str,
        name_alias: &str,
        codeword: &str,
    ) -> Self {
        Import {
            orig_module: orig_module.to_string(),
            orig_name: orig_name.to_string(),
            module_alias: module_alias.to_string(),
            name_alias: name_alias.to_string(),
            codeword: codeword.to_string(),
        }
    }
}
