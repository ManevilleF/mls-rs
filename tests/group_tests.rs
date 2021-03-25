use mls::group::Group;
use mls::rand::OpenSslRng;
use mls::client::Client;
use mls::extension::Lifetime;
use mls::credential::{Credential, BasicCredential};
use mls::signature::ed25519::EdDsa25519;
use mls::signature::SignatureScheme;
use mls::asym::AsymmetricKey;
use mls::key_package::{KeyPackageGenerator, KeyPackageGeneration};
use mls::ciphersuite::CipherSuite;
use mls::ciphersuite::CipherSuite::{
    MLS10_128_DHKEMX25519_AES128GCM_SHA256_Ed25519,
    MLS10_256_DHKEMP521_AES256GCM_SHA512_P521,
    MLS10_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519,
    MLS10_128_DHKEMP256_AES128GCM_SHA256_P256,
};

fn generate_client(id: Vec<u8>) -> Client {
    let signature_scheme = EdDsa25519::new_random(OpenSslRng).unwrap();
    let signature_key = signature_scheme.as_public_signature_key().unwrap();
    let basic = BasicCredential {
        signature_key: signature_key.signature_key,
        identity: id,
        signature_scheme: signature_key.signature_scheme
    };

    Client {
        signature_key: signature_scheme.get_signer().to_bytes().unwrap(),
        credential: Credential::Basic(basic),
        capabilities: Default::default(),
        key_lifetime: Lifetime { not_before: 0, not_after: 0 }
    }
}

fn test_create(cipher_suite: CipherSuite, update_path: bool) {
    let mut rng = OpenSslRng;

    let alice = generate_client(b"alice".to_vec());
    let bob = generate_client(b"bob".to_vec());

    let alice_key = alice.gen_key_package(&mut rng, &cipher_suite).unwrap();
    let bob_key = bob.gen_key_package(&mut rng, &cipher_suite).unwrap();

    // Alice creates a group and adds bob to the group
    let mut test_group = Group::new(
        &mut rng,
        b"group".to_vec(),
        alice_key.clone()
    ).unwrap();

    let add_members = test_group.add_member_proposals(
        &vec![bob_key.key_package.clone()]
    ).unwrap();

    let commit = test_group.commit_proposals(
        add_members,
        update_path,
        &mut rng,
        &alice
    ).unwrap();

    // Upon server confirmation, alice applies the commit to her own state
    test_group.process_pending_commit(commit.clone()).unwrap();

    // Bob receives the welcome message and joins the group
    let bob_group = Group::from_welcome_message(
        commit.welcome.unwrap(),
        test_group.public_tree.clone(),
        bob_key
    ).unwrap();

    assert_eq!(test_group, bob_group);
}

fn get_cipher_suites() -> Vec<CipherSuite> {
    [
        MLS10_128_DHKEMX25519_AES128GCM_SHA256_Ed25519,
        MLS10_256_DHKEMP521_AES256GCM_SHA512_P521,
        MLS10_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519,
        MLS10_128_DHKEMP256_AES128GCM_SHA256_P256
    ].to_vec()
}

#[test]
fn test_create_group_no_update() {
    get_cipher_suites()
        .iter()
        .for_each(|&cs| test_create(cs, false))
}

#[test]
fn test_create_group_update() {
    get_cipher_suites()
        .iter()
        .for_each(|&cs| test_create(cs, true))
}

fn test_path_updates(cipher_suite: CipherSuite) {
    // Create the group with Alice as the group initiator
    let alice = generate_client(b"alice".to_vec());

    let alice_key = alice.gen_key_package(&mut OpenSslRng, &cipher_suite)
        .unwrap();

    let mut test_group = Group::new(&mut OpenSslRng, b"group".to_vec(),
                                    alice_key.clone()).unwrap();

    // Generate 10 random clients that will be members of the group
    let clients = (0..10).into_iter()
        .map(|_| generate_client(b"test".to_vec())).collect::<Vec<Client>>();

    let test_keys = clients.iter()
        .map(|client| client.gen_key_package(&mut OpenSslRng, &cipher_suite).unwrap())
        .collect::<Vec<KeyPackageGeneration>>();

    // Add the generated clients to the group Alice created
    let add_members_proposal = test_group
        .add_member_proposals(&test_keys
            .iter()
            .map(|g| g.key_package.clone()).collect())
        .unwrap();

    let commit = test_group.commit_proposals(
        add_members_proposal,
        true,
        &mut OpenSslRng,
        &alice)
        .unwrap();

    test_group.process_pending_commit(commit.clone()).unwrap();

    // Create groups for each participant by processing Alice's welcome message
    let mut receiver_groups = test_keys
        .iter()
        .map(|kp|
            Group::from_welcome_message(commit.welcome.as_ref().unwrap().clone(),
                                        test_group.public_tree.clone(),
                                        kp.clone())
                .unwrap())
        .collect::<Vec<Group>>();


    // Loop through each participant and send a path update
    for i in 0..receiver_groups.len() {
        let pending = receiver_groups[i].commit_proposals(
            vec![],
            true,
            &mut OpenSslRng,
            &clients[i]
        ).unwrap();

        test_group.process_plaintext(pending.plaintext.clone()).unwrap();

        for j in 0..receiver_groups.len() {
            if i != j {
                receiver_groups[j].process_plaintext(pending.plaintext.clone()).unwrap();
            } else {
                receiver_groups[j].process_pending_commit(pending.clone()).unwrap();
            }
        }
    }

    // Validate that all the groups are in the same end state
    receiver_groups.iter().for_each(|group| assert_eq!(group, &test_group));
}

#[test]
fn test_group_path_updates() {
    get_cipher_suites()
        .iter()
        .for_each(|&cs| test_path_updates(cs))
}