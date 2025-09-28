use std::fs;

pub fn list_all_files(base_path: &str) -> Vec<String> {
    fs::read_dir(base_path).unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_list_all_files() {
        let files = list_all_files(".");
        assert!(files.len() > 0);
    }
}