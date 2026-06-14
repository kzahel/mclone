#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderBackend {
    NativeWgpu,
    WebWgpu,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_native_and_web_backends() {
        assert_ne!(RenderBackend::NativeWgpu, RenderBackend::WebWgpu);
    }
}
