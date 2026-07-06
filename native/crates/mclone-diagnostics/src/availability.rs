use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticLaneAvailability {
    #[default]
    Local,
    RemoteHost,
    Unsupported,
}

impl DiagnosticLaneAvailability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::RemoteHost => "remote-host",
            Self::Unsupported => "unsupported",
        }
    }
}
