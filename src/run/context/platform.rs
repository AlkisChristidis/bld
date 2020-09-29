use crate::run::{Machine, Container};
use std::fmt::{self, Display, Formatter};

pub enum RunPlatform {
    Local(Machine),
    Docker(Container),
}

impl Display for RunPlatform {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(_) => write!(f, "machine"),
            Self::Docker(container) => write!(f, "docker [ {} ]", container.image),
        }
    }
}