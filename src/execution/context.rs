#[derive(Debug, Clone)]
pub enum IterIndex<'a> {
    Array(usize),
    Map(&'a str),
}

#[derive(Debug, Clone)]
pub struct IterContext<'a> {
    pub iter_name: &'a str,
    pub index: IterIndex<'a>,
    pub depth: usize,
}

impl std::fmt::Display for IterIndex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IterIndex::Array(i) => write!(f, "{}", i),
            IterIndex::Map(k) => write!(f, "{}", k),
        }
    }
}

impl std::fmt::Display for IterContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]", self.iter_name, self.index)
    }
}
