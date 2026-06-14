use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    status: String,
    data: T,
    message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            status: String::from("ok"),
            data,
            message: None,
        }
    }
    pub fn success(data: T) -> Self {
        Self {
            status: String::from("success"),
            data,
            message: None,
        }
    }
}
