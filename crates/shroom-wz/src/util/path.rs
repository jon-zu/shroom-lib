/// Simple path traversr
/// a/b/c/d -> a, b, c, d
/// also informats about the last part
pub struct PathTraverser<'a> {
    p: &'a str,
    next_split: Option<usize>,
}

const DELIM: char = '/';

impl<'a> PathTraverser<'a> {
    pub fn new(path: &'a str) -> Self {
        Self {
            next_split: path.find(DELIM),
            p: path,
        }
    }

    pub fn next_segment(&mut self) -> Option<&'a str> {
        let next = self.next_split.take()?;
        let (path, tail) = self.p.split_at(next);
        self.p = &tail[1..];
        self.next_split = self.p.find(DELIM);

        Some(path)
    }

    pub fn remaining(&self) -> &'a str {
        self.p
    }
}

impl<'a> Iterator for PathTraverser<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_segment()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path() {
        assert_eq!(
            PathTraverser::new("a/b/c").collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }
}
