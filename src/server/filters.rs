pub mod filters {
    pub fn urlencode<T: std::fmt::Display>(s: T) -> ::askama::Result<String> {
        let string = s.to_string();
        // Since we are applying this to full paths like .app_data/..., we shouldn't encode slashes
        // Let's just manually replace spaces and other problematic chars if needed, or simply urlencode components
        // A simple fix for full paths is to split by `/`, encode, and rejoin.
        let encoded: Vec<String> = string.split('/')
            .map(|part| urlencoding::encode(part).into_owned())
            .collect();
        Ok(encoded.join("/"))
    }

    pub fn round(val: i64) -> ::askama::Result<i64> {
        Ok(val)
    }
}
