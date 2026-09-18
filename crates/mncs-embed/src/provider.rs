//! Generic authoritative provider admission for native sessions.
//!
//! The language/runtime owns the admission mechanism, not family routing.
//! A caller supplies a repository-owned descriptor and a session compiled
//! from the descriptor's admitted source. The caller must establish the
//! descriptor's claimed facts before constructing [`AdmittedProvider`]. The
//! registry then exposes only an exact identity-keyed, typed call boundary to
//! the consumer execution session.

use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{BodyType, ExecutionValue, ProviderCall, ProviderRuntime};
use serde::{Deserialize, Serialize};

use crate::{CallOptions, EmbedError, Grant, Session};

pub const PROVIDER_DESCRIPTOR_SCHEMA_VERSION: &str = "mncs.provider-descriptor/1";

/// Repository-owned provider selection facts. These are claims until the
/// admission owner compares them with compiled/source material and identity
/// entrypoints. Runtime invocation uses the established values only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFacts {
    pub descriptor_identity: String,
    pub provider_identity: String,
    pub interface_identity: String,
    pub source_identity: String,
    pub revision_identity: String,
    pub inventory_identity: String,
    pub artifact_identity: String,
    pub artifact_sha256: String,
}

impl ProviderFacts {
    fn validate(&self) -> Result<(), EmbedError> {
        for (label, value) in [
            ("provider_identity", &self.provider_identity),
            ("interface_identity", &self.interface_identity),
            ("source_identity", &self.source_identity),
            ("revision_identity", &self.revision_identity),
            ("inventory_identity", &self.inventory_identity),
        ] {
            validate_identity(label, value)?;
        }
        if self.descriptor_identity.is_empty() || self.artifact_identity.is_empty() {
            return Err(EmbedError::new(
                "invalid_provider_facts",
                "provider descriptor/artifact identity must be non-empty",
            ));
        }
        validate_identity("artifact_sha256", &self.artifact_sha256)?;
        Ok(())
    }
}

/// A provider whose source, interface, and identity facts have already been
/// established. It is deliberately not a general dynamic library handle:
/// one fixed entrypoint, one exact nominal protocol, and bounded explicit
/// grants are all that cross into a consumer session.
pub struct AdmittedProvider {
    facts: ProviderFacts,
    session: Session,
    entry_module: String,
    entry_function: String,
    grants: Vec<Grant>,
}

impl AdmittedProvider {
    pub fn new(
        session: Session,
        facts: ProviderFacts,
        entry_module: impl Into<String>,
        entry_function: impl Into<String>,
        grants: Vec<Grant>,
    ) -> Result<Self, EmbedError> {
        facts.validate()?;
        let entry_module = entry_module.into();
        let entry_function = entry_function.into();
        if entry_module.is_empty() || entry_function.is_empty() {
            return Err(EmbedError::new(
                "invalid_provider_facts",
                "admitted provider entry module/function must be non-empty",
            ));
        }
        let Some(interface_identity) = session.interface_identity() else {
            return Err(EmbedError::new(
                "invalid_provider_facts",
                "admitted provider artifact has no interface identity",
            ));
        };
        if interface_identity != facts.interface_identity {
            return Err(EmbedError::new(
                "provider_interface_mismatch",
                format!(
                    "provider descriptor interface {} does not match admitted artifact {}",
                    facts.interface_identity, interface_identity
                ),
            ));
        }
        if session.artifact_identity().is_empty() || session.digest() != facts.artifact_sha256 {
            return Err(EmbedError::new(
                "provider_artifact_mismatch",
                "provider artifact digest does not match established facts",
            ));
        }
        Ok(Self {
            facts,
            session,
            entry_module,
            entry_function,
            grants,
        })
    }

    pub fn facts(&self) -> &ProviderFacts {
        &self.facts
    }
}

/// Identity-keyed collection of admitted providers for one consumer
/// execution. Admission is explicit and bounded; no provider is discovered
/// by string search or downloaded by the runtime.
pub struct ProviderRegistry {
    providers: BTreeMap<String, AdmittedProvider>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: BTreeMap::new(),
        }
    }

    pub fn admit(&mut self, provider: AdmittedProvider) -> Result<(), EmbedError> {
        let identity = provider.facts.provider_identity.clone();
        if self.providers.contains_key(&identity) {
            return Err(EmbedError::new(
                "duplicate_provider_identity",
                format!("provider {identity} was admitted more than once"),
            ));
        }
        self.providers.insert(identity, provider);
        Ok(())
    }

    pub fn admitted_count(&self) -> usize {
        self.providers.len()
    }

    pub fn admitted_facts(&self) -> Vec<&ProviderFacts> {
        self.providers
            .values()
            .map(AdmittedProvider::facts)
            .collect()
    }
}

impl ProviderRuntime for ProviderRegistry {
    fn invoke(
        &self,
        provider_identity: &[u8],
        argument: ExecutionValue,
        expected: &BodyType,
        step_budget: u64,
    ) -> Result<ProviderCall, String> {
        if provider_identity.len() != 32 {
            return Err("provider identity must be exactly 32 bytes".to_owned());
        }
        let identity = encode_identity(provider_identity);
        let Some(provider) = self.providers.get(&identity) else {
            return Err(format!("no admitted provider matches identity {identity}"));
        };
        let options = CallOptions {
            step_budget,
            grants: provider.grants.clone(),
            expected_interface_identity: Some(provider.facts.interface_identity.clone()),
            type_arguments: Vec::new(),
        };
        let output = provider.session.call(
            &provider.entry_module,
            &provider.entry_function,
            vec![argument],
            &options,
        );
        if output.status != "returned" {
            return Err(output
                .failure_reason
                .unwrap_or_else(|| format!("provider returned {}", output.status)));
        }
        let mut returned = output.returned.into_iter();
        let Some(value) = returned.next() else {
            return Err("provider returned no typed value".to_owned());
        };
        if returned.next().is_some() {
            return Err("provider returned more than one typed value".to_owned());
        }
        let _ = expected;
        Ok(ProviderCall {
            value,
            effects: output.effects,
        })
    }
}

/// Descriptor consumed by a host launcher. Its fields are deliberately
/// declarative: source/library resolution is performed by the admission
/// owner, while identity entrypoints let the provider establish its own
/// nominal facts instead of treating descriptor claims as proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    pub schema_version: String,
    pub repository_id: String,
    pub provider_identity: String,
    pub interface_identity: String,
    pub source_identity: String,
    pub revision_identity: String,
    pub inventory_identity: String,
    pub descriptor_identity: String,
    pub source: String,
    pub module: String,
    pub entry_function: String,
    pub profile: String,
    #[serde(default)]
    pub libraries: Vec<String>,
    pub identity_entrypoints: BTreeMap<String, String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

#[derive(Serialize)]
struct ProviderDescriptorIdentityMaterial<'a> {
    schema_version: &'a str,
    repository_id: &'a str,
    provider_identity: &'a str,
    interface_identity: &'a str,
    source_identity: &'a str,
    revision_identity: &'a str,
    inventory_identity: &'a str,
    source: &'a str,
    module: &'a str,
    entry_function: &'a str,
    profile: &'a str,
    libraries: &'a [String],
    identity_entrypoints: &'a BTreeMap<String, String>,
    required_capabilities: &'a [String],
}

impl ProviderDescriptor {
    pub fn computed_identity(&self) -> String {
        let material = ProviderDescriptorIdentityMaterial {
            schema_version: &self.schema_version,
            repository_id: &self.repository_id,
            provider_identity: &self.provider_identity,
            interface_identity: &self.interface_identity,
            source_identity: &self.source_identity,
            revision_identity: &self.revision_identity,
            inventory_identity: &self.inventory_identity,
            source: &self.source,
            module: &self.module,
            entry_function: &self.entry_function,
            profile: &self.profile,
            libraries: &self.libraries,
            identity_entrypoints: &self.identity_entrypoints,
            required_capabilities: &self.required_capabilities,
        };
        mncs_model::sha256_hex(
            &serde_json::to_vec(&material).expect("provider descriptor material serializes"),
        )
    }

    pub fn validate(&self) -> Result<(), EmbedError> {
        if self.schema_version != PROVIDER_DESCRIPTOR_SCHEMA_VERSION {
            return Err(EmbedError::new(
                "invalid_provider_descriptor",
                format!(
                    "unsupported provider descriptor schema {}",
                    self.schema_version
                ),
            ));
        }
        if self.repository_id.is_empty()
            || self.source.is_empty()
            || self.module.is_empty()
            || self.entry_function.is_empty()
            || self.profile.is_empty()
        {
            return Err(EmbedError::new(
                "invalid_provider_descriptor",
                "provider descriptor has an empty routing field",
            ));
        }
        for (label, value) in [
            ("provider_identity", &self.provider_identity),
            ("interface_identity", &self.interface_identity),
            ("source_identity", &self.source_identity),
            ("revision_identity", &self.revision_identity),
            ("inventory_identity", &self.inventory_identity),
        ] {
            validate_identity(label, value)?;
        }
        if self.descriptor_identity != self.computed_identity() {
            return Err(EmbedError::new(
                "provider_descriptor_mismatch",
                "provider descriptor identity does not match its declared material",
            ));
        }
        for key in ["provider", "interface", "revision", "inventory"] {
            let Some(value) = self.identity_entrypoints.get(key) else {
                return Err(EmbedError::new(
                    "invalid_provider_descriptor",
                    format!("provider descriptor omits {key} identity entrypoint"),
                ));
            };
            if value.is_empty() || value.len() > 256 {
                return Err(EmbedError::new(
                    "invalid_provider_descriptor",
                    format!("provider descriptor {key} identity entrypoint is invalid"),
                ));
            }
        }
        let mut seen = BTreeSet::new();
        if self
            .required_capabilities
            .iter()
            .any(|capability| capability.is_empty() || !seen.insert(capability))
        {
            return Err(EmbedError::new(
                "invalid_provider_descriptor",
                "provider descriptor has duplicate or empty capabilities",
            ));
        }
        Ok(())
    }
}

pub fn decode_identity(value: &str) -> Result<Vec<u8>, EmbedError> {
    if !is_identity(value) {
        return Err(EmbedError::new(
            "invalid_identity",
            format!("{value:?} is not a lowercase 32-byte identity"),
        ));
    }
    Ok(value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0]).expect("validated identity");
            let low = hex_digit(pair[1]).expect("validated identity");
            (high << 4) | low
        })
        .collect())
}

pub fn encode_identity(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_identity(label: &str, value: &str) -> Result<(), EmbedError> {
    if !is_identity(value) {
        return Err(EmbedError::new(
            "invalid_identity",
            format!("{label} must be a lowercase 32-byte identity"),
        ));
    }
    Ok(())
}

fn is_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| hex_digit(*byte).is_some())
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}
