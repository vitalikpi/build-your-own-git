use crate::object_store;
use crate::object_store::{ObjectType, Sha};

#[derive(PartialEq)]
pub struct Commit {
    pub tree_sha: Sha,
    pub parent_sha: Option<Sha>,
    pub message: Option<String>,
    pub commiter: Option<String>
}

impl Commit {
    /* TODO:
    pub fn read(sha: String) -> anyhow::Result<Self> {
        let (object_type, _, content) = read_object(&sha)?;
        assert!(object_type == ObjectType::Commit);

        let tree_sha: String;
        let parent_sha: Option<String>;
        let message: String;
        let commiter: String;

        Self {
            tree_sha
            parent_sha,
            message,
            commiter
        }
    }
    */

    pub fn store(&self) -> anyhow::Result<Sha> {
        // tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904
        // parent 3b18e512dba79e4c8300dd08aeb37f8e728b8dad
        // author John Doe <john@example.com> 1234567890 +0000
        // committer John Doe <john@example.com> 1234567890 +0000
        //
        // Initial commit
        let mut commit_content: String = "tree ".to_string();

        commit_content.push_str(&self.tree_sha.to_string()); // TODO: is this correct?
        commit_content.push_str("\n");

        match &self.parent_sha {
            Some(parent_sha) => {
                commit_content.push_str("parent ");
                commit_content.push_str(&parent_sha.to_string()); // TODO: is this correct?
                commit_content.push_str("\n");
            }
            None => {}
        }

        commit_content.push_str("author John Doe <john@example.com> 1234567890 +0000");
        commit_content.push_str("\n");
        commit_content.push_str("\n");

        match &self.message {
            Some(message) => {
                commit_content.push_str(&*message);
                commit_content.push_str("\n");
            }
            None => {}
        }

        object_store::write_object(commit_content.into_bytes(), ObjectType::Commit)
    }
}