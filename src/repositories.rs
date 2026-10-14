//! Private Cloud Hub: information about your repositories.

use serde::{Deserialize, Serialize};

/// Information about a Private Cloud Hub repository, from
/// [`Client::repository_info`](crate::Client::repository_info). Missing fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RepositoryInfo {
    /// The repository id.
    pub id: Option<String>,
    /// The repository name.
    pub name: Option<String>,
    /// The repository description.
    pub description: Option<String>,
    /// Your role in the repository: `1` viewer, `2` contributor, `3` admin, `4` super admin.
    /// See [`RepositoryRole::from_value`].
    pub permission: Option<i64>,
    /// The repository status: `0` running, `1` pending deletion, `2` error, `3` updating,
    /// `4` payment required. See [`RepositoryStatus::from_value`].
    pub status: Option<i64>,
    /// The credit limit of the repository.
    pub max_credits: Option<i64>,
    /// The credits the repository has used.
    pub current_credits: Option<i64>,
    /// The masking policy of the repository: `0` no masking, `1` mask other users' documents,
    /// `2` mask all documents. See
    /// [`MaskingPolicy::from_value`](crate::plagiarism::MaskingPolicy::from_value).
    pub masking_policy: Option<i64>,
    /// When the repository was created, as an ISO 8601 time.
    pub creation_time: Option<String>,
}

int_enum! {
    /// A user's role in a Private Cloud Hub ([`RepositoryInfo::permission`]).
    RepositoryRole {
        /// Reads part of the metadata and scans against the documents.
        Viewer = 1,
        /// A viewer who can also add documents and delete their own.
        Contributor = 2,
        /// A contributor who can also invite users and delete any document.
        Admin = 3,
        /// An admin who can also delete the repository and manage billing.
        SuperAdmin = 4,
    }
}

int_enum! {
    /// The status of a Private Cloud Hub ([`RepositoryInfo::status`]).
    RepositoryStatus {
        /// The repository is healthy.
        Running = 0,
        /// The repository is being deleted, with all its documents, and cannot be used.
        PendingForDeletion = 1,
        /// The repository is not healthy. Contact Copyleaks support.
        Error = 2,
        /// Maintenance is running; the repository may be unavailable.
        Updating = 3,
        /// The repository is suspended because of a missing or failed payment.
        PaymentRequired = 4,
    }
}
