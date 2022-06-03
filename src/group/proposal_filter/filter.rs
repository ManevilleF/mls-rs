use crate::{
    group::{proposal_filter::ProposalBundle, ProposalType, Sender},
    key_package::KeyPackageValidationError,
    tree_kem::{
        leaf_node::LeafNodeError, leaf_node_validator::LeafNodeValidationError, node::LeafIndex,
        RatchetTreeError,
    },
    ProtocolVersion,
};
use thiserror::Error;

pub trait ProposalFilter {
    type Error: std::error::Error + Send + Sync + 'static;

    fn validate(&self, proposals: &ProposalBundle) -> Result<(), Self::Error>;
    fn filter(&self, proposals: ProposalBundle) -> Result<ProposalBundle, Self::Error>;

    fn and<T>(self, other: T) -> And<Self, T>
    where
        Self: Sized,
        T: ProposalFilter<Error = Self::Error>,
    {
        And(self, other)
    }
}

#[derive(Clone, Debug)]
pub struct And<A, B>(A, B);

impl<A, B> ProposalFilter for And<A, B>
where
    A: ProposalFilter,
    B: ProposalFilter<Error = A::Error>,
{
    type Error = A::Error;

    fn validate(&self, proposals: &ProposalBundle) -> Result<(), Self::Error> {
        self.0.validate(proposals)?;
        self.1.validate(proposals)?;
        Ok(())
    }

    fn filter(&self, proposals: ProposalBundle) -> Result<ProposalBundle, Self::Error> {
        self.1.filter(self.0.filter(proposals)?)
    }
}

#[derive(Debug, Error)]
pub enum ProposalFilterError {
    #[error(transparent)]
    KeyPackageValidationError(#[from] KeyPackageValidationError),
    #[error(transparent)]
    LeafNodeValidationError(#[from] LeafNodeValidationError),
    #[error(transparent)]
    RatchetTreeError(#[from] RatchetTreeError),
    #[error(transparent)]
    LeafNodeError(#[from] LeafNodeError),
    #[error("Commiter must not include any update proposals generated by the commiter")]
    InvalidCommitSelfUpdate,
    #[error("PSK type must be External in PreSharedKey proposal")]
    PskTypeMustBeExternalInPreSharedKeyProposal,
    #[error("Expected PSK nonce with length {expected} but found length {found}")]
    InvalidPskNonceLength { expected: usize, found: usize },
    #[error("Protocol version {proposed:?} in ReInit proposal is less than version {original:?} in original group")]
    InvalidProtocolVersionInReInit {
        proposed: ProtocolVersion,
        original: ProtocolVersion,
    },
    #[error("More than one proposal applying to leaf {0:?}")]
    MoreThanOneProposalForLeaf(LeafIndex),
    #[error("More than one GroupContextExtensions proposal")]
    MoreThanOneGroupContextExtensionsProposal,
    #[error("Invalid proposal of type {0:?} for proposer {1:?}")]
    InvalidProposalTypeForProposer(ProposalType, Sender),
    #[error("External commit must have exactly one ExternalInit proposal")]
    ExternalCommitMustHaveExactlyOneExternalInit,
    #[error("Preconfigured sender cannot commit")]
    PreconfiguredSenderCannotCommit,
    #[error("Missing update path in external commit")]
    MissingUpdatePathInExternalCommit,
    #[error("External commit contains removal of other identity")]
    ExternalCommitRemovesOtherIdentity,
    #[error("External commit contains more than one Remove proposal")]
    ExternalCommitWithMoreThanOneRemove,
    #[error("Duplicate PSK IDs")]
    DuplicatePskIds,
    #[error("ExternalInit must be committed by NewMember")]
    ExternalInitMustBeCommittedByNewMember,
    #[error("Invalid proposal type {0:?} in external commit")]
    InvalidProposalTypeInExternalCommit(ProposalType),
    #[error("Committer can not remove themselves")]
    CommitterSelfRemoval,
}
