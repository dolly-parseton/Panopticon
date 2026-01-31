pub mod context;
pub mod helpers;
pub mod scalar;
pub mod tabular;

/*
    StorePath - Represents a path to a value in the store, e.g., "namespace.key.subkey"
*/
pub mod store_path {
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
    pub struct StorePath {
        segments: Vec<String>,
    }

    impl StorePath {
        pub fn from_segments(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
            StorePath {
                segments: segments.into_iter().map(|s| s.into()).collect(),
            }
        }
        pub fn add_segment(&mut self, segment: impl Into<String>) {
            self.segments.push(segment.into());
        }

        pub fn with_segment(&self, segment: impl Into<String>) -> Self {
            let mut new_path = self.clone();
            new_path.segments.push(segment.into());
            new_path
        }
        pub fn with_index(&self, index: usize) -> Self {
            self.with_segment(index.to_string())
        }
        pub fn to_dotted(&self) -> String {
            self.segments.join(".")
        }
        pub fn from_dotted(dotted: &str) -> Self {
            let segments = dotted
                .split('.')
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            StorePath { segments }
        }
        pub fn segments(&self) -> &[String] {
            &self.segments
        }
        pub fn namespace(&self) -> Option<&String> {
            self.segments.first()
        }
        pub fn starts_with(&self, other: &StorePath) -> bool {
            if other.segments.len() > self.segments.len() {
                return false;
            }
            for (a, b) in self.segments.iter().zip(other.segments.iter()) {
                if a != b {
                    return false;
                }
            }
            true
        }
        pub fn contains(&self, segment: &str) -> bool {
            self.segments.iter().any(|s| s == segment)
        }
    }

    impl std::fmt::Display for StorePath {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.to_dotted())
        }
    }
}
