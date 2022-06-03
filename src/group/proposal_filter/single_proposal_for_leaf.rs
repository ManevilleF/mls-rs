use crate::group::{
    proposal_filter::{ProposalBundle, ProposalFilter, ProposalFilterError},
    RemoveProposal, Sender, UpdateProposal,
};
use std::collections::HashSet;

#[derive(Debug)]
pub struct SingleProposalForLeaf;

impl ProposalFilter for SingleProposalForLeaf {
    type Error = ProposalFilterError;

    fn validate(&self, proposals: &ProposalBundle) -> Result<(), Self::Error> {
        proposals
            .by_type::<RemoveProposal>()
            .map(|p| p.proposal.to_remove)
            .chain(
                proposals
                    .by_type::<UpdateProposal>()
                    .filter_map(|p| match &p.sender {
                        Sender::Member(leaf) => Some(*leaf),
                        _ => None,
                    }),
            )
            .try_fold(HashSet::new(), |mut leaves, leaf| {
                leaves
                    .insert(leaf)
                    .then(|| leaves)
                    .ok_or(ProposalFilterError::MoreThanOneProposalForLeaf(leaf))
            })
            .map(|_| ())
    }

    fn filter(&self, mut proposals: ProposalBundle) -> Result<ProposalBundle, Self::Error> {
        let mut leaves = HashSet::new();

        proposals.retain_by_type::<RemoveProposal, _>(|p| leaves.insert(p.proposal.to_remove));

        proposals.retain_by_type::<UpdateProposal, _>(|p| match &p.sender {
            Sender::Member(leaf) => leaves.insert(*leaf),
            _ => true,
        });

        Ok(proposals)
    }
}
