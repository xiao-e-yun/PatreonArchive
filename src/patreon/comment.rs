pub use parent::Comment;

use jsonapi_deserialize::JsonApiDeserialize;

#[derive(Debug, Clone, JsonApiDeserialize)]
#[json_api(rename_all = "snake_case")]
pub struct User {
    pub id: String,
    pub image_url: String,
    pub full_name: String,
    pub url: String,
}

mod parent {
    use std::{ops::Deref, sync::Arc};

    use chrono::{DateTime, Utc};
    use jsonapi_deserialize::JsonApiDeserialize;

    use super::User;

    #[derive(Debug, Clone, JsonApiDeserialize)]
    #[json_api(rename_all = "snake_case")]
    pub struct Comment {
        pub body: String,
        pub created: DateTime<Utc>,
        #[json_api(relationship = "single", resource = "User")]
        pub commenter: Arc<User>,
        #[json_api(relationship = "multiple", resource = "super::child::Comment")]
        pub replies: Vec<Arc<super::child::Comment>>,
    }

    impl From<Comment> for post_archiver::Comment {
        fn from(val: Comment) -> Self {
            Self {
                user: val.commenter.full_name.clone(),
                text: val.body,
                replies: val
                    .replies
                    .into_iter()
                    .map(|e| e.deref().clone().into())
                    .collect(),
            }
        }
    }
}

mod child {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use jsonapi_deserialize::JsonApiDeserialize;

    use super::User;

    #[derive(Debug, Clone, JsonApiDeserialize)]
    #[json_api(rename_all = "snake_case")]
    pub struct Comment {
        pub body: String,
        pub created: DateTime<Utc>,
        #[json_api(relationship = "single", resource = "User")]
        pub commenter: Arc<User>,
    }

    impl From<Comment> for post_archiver::Comment {
        fn from(val: Comment) -> Self {
            Self {
                user: val.commenter.full_name.clone(),
                text: val.body,
                replies: vec![],
            }
        }
    }
}
