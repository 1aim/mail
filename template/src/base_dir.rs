use std::{
    env, io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CwdBaseDir(PathBuf);

impl CwdBaseDir {
    /// Creates a new `CwdBaseDir` instance containing exactly the given path.
    pub fn new_unchanged(path: PathBuf) -> Self {
        CwdBaseDir(path)
    }

    /// Creates a `CwdBaseDir` from a path by prefixing the path with the
    /// current working dir if it's relative.
    ///
    /// If the path is not relative it's directly used.
    ///
    /// # Os state side effects
    ///
    /// As this function accesses the current working directory (CWD) it's
    /// not pure as the CWD can be changed (e.g. by `std::env::set_current_dir`).
    ///
    /// # Error
    ///
    /// As getting the CWD can fail this function can fail with a I/O Error, too.
    pub fn from_path<P>(path: P) -> Result<Self, io::Error>
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        let path = if path.as_ref().is_absolute() {
            path.into()
        } else {
            let mut cwd = env::current_dir()?;
            cwd.push(path.as_ref());
            cwd
        };

        Ok(CwdBaseDir(path))
    }

    /// Turns this path into a `PathBuf` by stripping the current working dir
    /// if it starts with it.
    ///
    /// If this path does not start with the CWD it's returned directly.
    ///
    /// # Os state side effects
    ///
    /// As this function used the  current working dir (CWD) it is affected
    /// by any function changing the CWD as a side effect.
    ///
    /// # Error
    ///
    /// Accessing the current working dir can fail, as such this function
    /// can fail.
    pub fn to_base_path(&self) -> Result<&Path, io::Error> {
        let cwd = env::current_dir()?;
        self.strip_prefix(&cwd)
            .or_else(|_err_does_not_has_that_prefix| Ok(&self))
    }

    /// Turns this instance into the `PathBuf` it dereferences to.
    pub fn into_inner_with_prefix(self) -> PathBuf {
        let CwdBaseDir(path) = self;
        path
    }
}

impl Deref for CwdBaseDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CwdBaseDir {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<Path> for CwdBaseDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CwdBaseDir {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let path_buf = PathBuf::deserialize(deserializer)?;
        Self::from_path(path_buf).map_err(|err| D::Error::custom(err))
    }
}

impl Serialize for CwdBaseDir {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        let path = self.to_base_path().map_err(|err| S::Error::custom(err))?;

        path.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_path_does_not_affect_absolute_paths() {
        let path = Path::new("/the/dog");
        let base_dir = CwdBaseDir::from_path(path).unwrap();
        assert_eq!(&*base_dir, Path::new("/the/dog"))
    }

    #[test]
    fn from_path_prefixes_with_cwd() {
        let cwd = env::current_dir().unwrap();
        let expected = cwd.join("./the/dog");

        let base_dir = CwdBaseDir::from_path("./the/dog").unwrap();
        assert_eq!(&*base_dir, &expected);
    }

    #[test]
    fn to_base_path_removes_cwd_prefix() {
        let cwd = env::current_dir().unwrap();
        let dir = cwd.join("hy/there");
        let base_dir = CwdBaseDir::new_unchanged(dir);
        let path = base_dir.to_base_path().unwrap();
        assert_eq!(path, Path::new("hy/there"));
    }
}
