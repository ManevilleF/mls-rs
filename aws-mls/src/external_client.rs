use crate::{
    cipher_suite::CipherSuite,
    client::MlsError,
    group::framing::MLSMessage,
    key_package::{KeyPackageValidationOptions, KeyPackageValidationOutput, KeyPackageValidator},
    protocol_version::ProtocolVersion,
    time::MlsTime,
    CryptoProvider, WireFormat,
};

pub mod builder;
mod config;
mod group;

pub(crate) use config::ExternalClientConfig;

use builder::{ExternalBaseConfig, ExternalClientBuilder};

pub use group::{ExternalGroup, ExternalReceivedMessage, ExternalSnapshot};

/// A client capable of observing a group's state without having
/// private keys required to read content.
///
/// This structure is useful when an application is sending
/// plaintext control messages in order to allow a central server
/// to facilitate communication between users.
///
/// # Warning
///
/// This structure will only be able to observe groups that were
/// created by clients that have the encrypt_controls
/// [preference](crate::client_builder::Preferences)
/// set to `false`. Any control messages that are sent encrypted
/// over the wire will break the ability of this client to track
/// the resulting group state.
pub struct ExternalClient<C> {
    config: C,
}

impl ExternalClient<()> {
    pub fn builder() -> ExternalClientBuilder<ExternalBaseConfig> {
        ExternalClientBuilder::new()
    }
}

impl<C> ExternalClient<C>
where
    C: ExternalClientConfig + Clone,
{
    pub(crate) fn new(config: C) -> Self {
        Self { config }
    }

    /// Begin observing a group based on a GroupInfo message created by
    /// [Group::group_info_message](crate::group::Group::group_info_message)
    ///
    ///`tree_data` is required to be provided out of band if the client that
    /// created GroupInfo message did not have the
    /// [ratchet tree extension preference](crate::client_builder::Preferences::ratchet_tree_extension)
    /// enabled at the time the welcome message was created. `tree_data` can
    /// be exported from a group using the
    /// [export tree function](crate::group::Group::export_tree).
    pub async fn observe_group(
        &self,
        group_info: MLSMessage,
        tree_data: Option<&[u8]>,
    ) -> Result<ExternalGroup<C>, MlsError> {
        ExternalGroup::join(self.config.clone(), group_info, tree_data)
            .await
            .map_err(Into::into)
    }

    /// Load an existing observed group by loading a snapshot that was
    /// generated by
    /// [ExternalGroup::snapshot](self::ExternalGroup::snapshot).
    pub async fn load_group(
        &self,
        snapshot: ExternalSnapshot,
    ) -> Result<ExternalGroup<C>, MlsError> {
        ExternalGroup::from_snapshot(self.config.clone(), snapshot)
            .await
            .map_err(Into::into)
    }

    /// Utility function to validate key packages
    pub async fn validate_key_package(
        &self,
        package: MLSMessage,
        protocol: ProtocolVersion,
        cipher_suite: CipherSuite,
    ) -> Result<KeyPackageValidationOutput, MlsError> {
        let wire_format = package.wire_format();

        let key_package = package.into_key_package().ok_or_else(|| {
            MlsError::UnexpectedMessageType(vec![WireFormat::KeyPackage], wire_format)
        })?;

        let cipher_suite_provider = self
            .config
            .crypto_provider()
            .cipher_suite_provider(cipher_suite)
            .ok_or_else(|| MlsError::UnsupportedCipherSuite(cipher_suite))?;

        let id_provider = self.config.identity_provider();

        let keypackage_validator =
            KeyPackageValidator::new(protocol, &cipher_suite_provider, None, &id_provider, None);

        let options = KeyPackageValidationOptions {
            apply_lifetime_check: Some(MlsTime::now()),
        };

        keypackage_validator
            .check_if_valid(&key_package, options)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
pub(crate) mod tests_utils {
    pub use super::builder::test_utils::*;
}
