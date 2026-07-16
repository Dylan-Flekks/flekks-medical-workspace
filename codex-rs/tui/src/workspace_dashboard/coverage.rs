use codex_app_server_protocol::WorkspaceBillingReadiness;
use codex_app_server_protocol::WorkspaceCoverageVerificationCreateParams;
use codex_app_server_protocol::WorkspaceCoverageVerificationSubject;

/// Explains the provenance boundary for coverage-card identity input.
///
/// These values are transcribed and confirmed by a person. The workspace does
/// not use OCR or a model to infer identity from the linked source document.
pub(super) const CARD_VERIFICATION_ENTRY_HELP: &str = "Type the identity exactly as printed on the card, then confirm it. No OCR or model inference is used.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoveragePriority {
    Primary,
    Secondary,
    Tertiary,
}

impl CoveragePriority {
    pub(super) const ALL: [Self; 3] = [Self::Primary, Self::Secondary, Self::Tertiary];

    pub(super) fn number(self) -> i64 {
        match self {
            Self::Primary => 1,
            Self::Secondary => 2,
            Self::Tertiary => 3,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
            Self::Tertiary => "Tertiary",
        }
    }

    pub(super) fn next_wrapping(self) -> Self {
        match self {
            Self::Primary => Self::Secondary,
            Self::Secondary => Self::Tertiary,
            Self::Tertiary => Self::Primary,
        }
    }

    pub(super) fn previous_wrapping(self) -> Self {
        match self {
            Self::Primary => Self::Tertiary,
            Self::Secondary => Self::Primary,
            Self::Tertiary => Self::Secondary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InvalidCoveragePriority(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum CoverageField {
    PayerName,
    PlanName,
    MemberId,
    GroupNumber,
    CoverageType,
    CoverageStatus,
    EffectiveDate,
    TerminationDate,
    PatientRelationshipToSubscriber,
    SubscriberFirstName,
    SubscriberMiddleName,
    SubscriberLastName,
    SubscriberSuffix,
    SubscriberDateOfBirth,
    SubscriberAdministrativeSex,
    SubscriberAddressSameAsPatient,
    SubscriberAddressLine1,
    SubscriberAddressLine2,
    SubscriberCity,
    SubscriberStateOrProvince,
    SubscriberPostalCode,
    SubscriberCountry,
    CoverageNotes,
}

impl CoverageField {
    pub(super) const ALL: [Self; 23] = [
        Self::PayerName,
        Self::PlanName,
        Self::MemberId,
        Self::GroupNumber,
        Self::CoverageType,
        Self::CoverageStatus,
        Self::EffectiveDate,
        Self::TerminationDate,
        Self::PatientRelationshipToSubscriber,
        Self::SubscriberFirstName,
        Self::SubscriberMiddleName,
        Self::SubscriberLastName,
        Self::SubscriberSuffix,
        Self::SubscriberDateOfBirth,
        Self::SubscriberAdministrativeSex,
        Self::SubscriberAddressSameAsPatient,
        Self::SubscriberAddressLine1,
        Self::SubscriberAddressLine2,
        Self::SubscriberCity,
        Self::SubscriberStateOrProvince,
        Self::SubscriberPostalCode,
        Self::SubscriberCountry,
        Self::CoverageNotes,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::PayerName => "Payer",
            Self::PlanName => "Plan",
            Self::MemberId => "Member ID",
            Self::GroupNumber => "Group number",
            Self::CoverageType => "Coverage type",
            Self::CoverageStatus => "Coverage status",
            Self::EffectiveDate => "Effective date",
            Self::TerminationDate => "Termination date",
            Self::PatientRelationshipToSubscriber => "Relationship to subscriber",
            Self::SubscriberFirstName => "Subscriber legal first name",
            Self::SubscriberMiddleName => "Subscriber legal middle name",
            Self::SubscriberLastName => "Subscriber legal last name",
            Self::SubscriberSuffix => "Subscriber legal suffix",
            Self::SubscriberDateOfBirth => "Subscriber date of birth",
            Self::SubscriberAdministrativeSex => "Subscriber administrative sex",
            Self::SubscriberAddressSameAsPatient => "Subscriber address same as patient",
            Self::SubscriberAddressLine1 => "Subscriber address line 1",
            Self::SubscriberAddressLine2 => "Subscriber address line 2",
            Self::SubscriberCity => "Subscriber city",
            Self::SubscriberStateOrProvince => "Subscriber state/province",
            Self::SubscriberPostalCode => "Subscriber postal code",
            Self::SubscriberCountry => "Subscriber country",
            Self::CoverageNotes => "Coverage notes",
        }
    }

    pub(super) fn next_wrapping(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    pub(super) fn previous_wrapping(self) -> Self {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub(super) fn is_toggle(self) -> bool {
        self == Self::SubscriberAddressSameAsPatient
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoverageDraft {
    pub id: Option<String>,
    pub version: Option<String>,
    pub client_id: String,
    pub priority: CoveragePriority,
    pub payer_name: String,
    pub plan_name: String,
    pub member_id: String,
    pub group_number: String,
    pub coverage_type: String,
    pub coverage_status: String,
    pub effective_date: String,
    pub termination_date: String,
    pub patient_relationship_to_subscriber: String,
    pub subscriber_first_name: String,
    pub subscriber_middle_name: String,
    pub subscriber_last_name: String,
    pub subscriber_suffix: String,
    pub subscriber_date_of_birth: String,
    pub subscriber_administrative_sex: String,
    pub subscriber_address_same_as_patient: bool,
    pub subscriber_address_line_1: String,
    pub subscriber_address_line_2: String,
    pub subscriber_city: String,
    pub subscriber_state_or_province: String,
    pub subscriber_postal_code: String,
    pub subscriber_country: String,
    pub coverage_notes: String,
    pub billing_readiness: WorkspaceBillingReadiness,
}

impl CoverageDraft {
    pub(super) fn new(client_id: impl Into<String>, priority: CoveragePriority) -> Self {
        Self {
            id: None,
            version: None,
            client_id: client_id.into(),
            priority,
            payer_name: String::new(),
            plan_name: String::new(),
            member_id: String::new(),
            group_number: String::new(),
            coverage_type: String::new(),
            coverage_status: String::new(),
            effective_date: String::new(),
            termination_date: String::new(),
            patient_relationship_to_subscriber: String::new(),
            subscriber_first_name: String::new(),
            subscriber_middle_name: String::new(),
            subscriber_last_name: String::new(),
            subscriber_suffix: String::new(),
            subscriber_date_of_birth: String::new(),
            subscriber_administrative_sex: String::new(),
            subscriber_address_same_as_patient: false,
            subscriber_address_line_1: String::new(),
            subscriber_address_line_2: String::new(),
            subscriber_city: String::new(),
            subscriber_state_or_province: String::new(),
            subscriber_postal_code: String::new(),
            subscriber_country: String::new(),
            coverage_notes: String::new(),
            billing_readiness: WorkspaceBillingReadiness::Incomplete,
        }
    }

    pub(super) fn text(&self, field: CoverageField) -> Option<&str> {
        match field {
            CoverageField::PayerName => Some(&self.payer_name),
            CoverageField::PlanName => Some(&self.plan_name),
            CoverageField::MemberId => Some(&self.member_id),
            CoverageField::GroupNumber => Some(&self.group_number),
            CoverageField::CoverageType => Some(&self.coverage_type),
            CoverageField::CoverageStatus => Some(&self.coverage_status),
            CoverageField::EffectiveDate => Some(&self.effective_date),
            CoverageField::TerminationDate => Some(&self.termination_date),
            CoverageField::PatientRelationshipToSubscriber => {
                Some(&self.patient_relationship_to_subscriber)
            }
            CoverageField::SubscriberFirstName => Some(&self.subscriber_first_name),
            CoverageField::SubscriberMiddleName => Some(&self.subscriber_middle_name),
            CoverageField::SubscriberLastName => Some(&self.subscriber_last_name),
            CoverageField::SubscriberSuffix => Some(&self.subscriber_suffix),
            CoverageField::SubscriberDateOfBirth => Some(&self.subscriber_date_of_birth),
            CoverageField::SubscriberAdministrativeSex => Some(&self.subscriber_administrative_sex),
            CoverageField::SubscriberAddressSameAsPatient => None,
            CoverageField::SubscriberAddressLine1 => Some(&self.subscriber_address_line_1),
            CoverageField::SubscriberAddressLine2 => Some(&self.subscriber_address_line_2),
            CoverageField::SubscriberCity => Some(&self.subscriber_city),
            CoverageField::SubscriberStateOrProvince => Some(&self.subscriber_state_or_province),
            CoverageField::SubscriberPostalCode => Some(&self.subscriber_postal_code),
            CoverageField::SubscriberCountry => Some(&self.subscriber_country),
            CoverageField::CoverageNotes => Some(&self.coverage_notes),
        }
    }

    pub(super) fn text_mut(&mut self, field: CoverageField) -> Option<&mut String> {
        match field {
            CoverageField::PayerName => Some(&mut self.payer_name),
            CoverageField::PlanName => Some(&mut self.plan_name),
            CoverageField::MemberId => Some(&mut self.member_id),
            CoverageField::GroupNumber => Some(&mut self.group_number),
            CoverageField::CoverageType => Some(&mut self.coverage_type),
            CoverageField::CoverageStatus => Some(&mut self.coverage_status),
            CoverageField::EffectiveDate => Some(&mut self.effective_date),
            CoverageField::TerminationDate => Some(&mut self.termination_date),
            CoverageField::PatientRelationshipToSubscriber => {
                Some(&mut self.patient_relationship_to_subscriber)
            }
            CoverageField::SubscriberFirstName => Some(&mut self.subscriber_first_name),
            CoverageField::SubscriberMiddleName => Some(&mut self.subscriber_middle_name),
            CoverageField::SubscriberLastName => Some(&mut self.subscriber_last_name),
            CoverageField::SubscriberSuffix => Some(&mut self.subscriber_suffix),
            CoverageField::SubscriberDateOfBirth => Some(&mut self.subscriber_date_of_birth),
            CoverageField::SubscriberAdministrativeSex => {
                Some(&mut self.subscriber_administrative_sex)
            }
            CoverageField::SubscriberAddressSameAsPatient => None,
            CoverageField::SubscriberAddressLine1 => Some(&mut self.subscriber_address_line_1),
            CoverageField::SubscriberAddressLine2 => Some(&mut self.subscriber_address_line_2),
            CoverageField::SubscriberCity => Some(&mut self.subscriber_city),
            CoverageField::SubscriberStateOrProvince => {
                Some(&mut self.subscriber_state_or_province)
            }
            CoverageField::SubscriberPostalCode => Some(&mut self.subscriber_postal_code),
            CoverageField::SubscriberCountry => Some(&mut self.subscriber_country),
            CoverageField::CoverageNotes => Some(&mut self.coverage_notes),
        }
    }

    pub(super) fn toggle_subscriber_address_same_as_patient(&mut self) {
        self.subscriber_address_same_as_patient = !self.subscriber_address_same_as_patient;
    }

    pub(super) fn concise_summary(&self) -> String {
        let payer = entered_or(&self.payer_name, "payer not entered");
        let plan = (!self.plan_name.trim().is_empty()).then(|| self.plan_name.trim());
        let product = plan.map_or_else(|| payer.to_string(), |plan| format!("{payer} / {plan}"));
        format!(
            "{} · {product} · {} · {}",
            self.priority.label(),
            member_id_summary(&self.member_id),
            billing_readiness_label(self.billing_readiness)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardVerificationDraft {
    pub coverage_id: String,
    pub source_document_id: String,
    pub expected_patient_version: String,
    pub expected_coverage_version: String,
    pub expected_document_version: String,
    pub compared_subject: WorkspaceCoverageVerificationSubject,
    pub observed_first_name: String,
    pub observed_middle_name: String,
    pub observed_last_name: String,
    pub observed_suffix: String,
    pub observed_member_id: String,
    pub actor: String,
}

impl CardVerificationDraft {
    pub(super) fn new(
        coverage_id: impl Into<String>,
        compared_subject: WorkspaceCoverageVerificationSubject,
    ) -> Self {
        Self {
            coverage_id: coverage_id.into(),
            source_document_id: String::new(),
            expected_patient_version: String::new(),
            expected_coverage_version: String::new(),
            expected_document_version: String::new(),
            compared_subject,
            observed_first_name: String::new(),
            observed_middle_name: String::new(),
            observed_last_name: String::new(),
            observed_suffix: String::new(),
            observed_member_id: String::new(),
            actor: String::new(),
        }
    }

    pub(super) fn toggle_compared_subject(&mut self) {
        self.compared_subject = match self.compared_subject {
            WorkspaceCoverageVerificationSubject::Beneficiary => {
                WorkspaceCoverageVerificationSubject::Subscriber
            }
            WorkspaceCoverageVerificationSubject::Subscriber => {
                WorkspaceCoverageVerificationSubject::Beneficiary
            }
        };
    }

    pub(super) fn submission_issue(&self) -> Option<&'static str> {
        if self.coverage_id.trim().is_empty() {
            return Some("Save the coverage before recording a card verification.");
        }
        if self.source_document_id.trim().is_empty() {
            return Some("Select the source card document.");
        }
        if self.expected_patient_version.trim().is_empty()
            || self.expected_coverage_version.trim().is_empty()
            || self.expected_document_version.trim().is_empty()
        {
            return Some("Reload the patient, coverage, and source card before verification.");
        }
        if self.observed_first_name.trim().is_empty() || self.observed_last_name.trim().is_empty() {
            return Some("Enter the first and last name exactly as printed on the card.");
        }
        if self.observed_member_id.trim().is_empty() {
            return Some("Enter the member ID exactly as printed on the card.");
        }
        if self.actor.trim().is_empty() {
            return Some("Identify the person who confirmed the card values.");
        }
        None
    }
}

impl From<&CardVerificationDraft> for WorkspaceCoverageVerificationCreateParams {
    fn from(value: &CardVerificationDraft) -> Self {
        Self {
            coverage_id: value.coverage_id.trim().to_string(),
            source_document_id: value.source_document_id.trim().to_string(),
            expected_patient_version: value.expected_patient_version.trim().to_string(),
            expected_coverage_version: value.expected_coverage_version.trim().to_string(),
            expected_document_version: value.expected_document_version.trim().to_string(),
            compared_subject: value.compared_subject,
            observed_first_name: optional_entry(&value.observed_first_name),
            observed_middle_name: optional_entry(&value.observed_middle_name),
            observed_last_name: optional_entry(&value.observed_last_name),
            observed_suffix: optional_entry(&value.observed_suffix),
            observed_member_id: optional_entry(&value.observed_member_id),
            actor: value.actor.trim().to_string(),
        }
    }
}

impl From<CardVerificationDraft> for WorkspaceCoverageVerificationCreateParams {
    fn from(value: CardVerificationDraft) -> Self {
        Self::from(&value)
    }
}

pub(super) fn verification_subject_label(
    subject: WorkspaceCoverageVerificationSubject,
) -> &'static str {
    match subject {
        WorkspaceCoverageVerificationSubject::Beneficiary => "Beneficiary on card",
        WorkspaceCoverageVerificationSubject::Subscriber => "Subscriber on card",
    }
}

pub(super) fn billing_readiness_label(readiness: WorkspaceBillingReadiness) -> &'static str {
    match readiness {
        WorkspaceBillingReadiness::Match => "MATCH · billing ready",
        WorkspaceBillingReadiness::Mismatch => "MISMATCH · billing/export blocked",
        WorkspaceBillingReadiness::Unverified => "UNVERIFIED · billing/export blocked",
        WorkspaceBillingReadiness::Stale => "STALE · verify again",
        WorkspaceBillingReadiness::Incomplete => "INCOMPLETE · finish identity",
    }
}

pub(super) fn billing_readiness_summary(readiness: WorkspaceBillingReadiness) -> &'static str {
    match readiness {
        WorkspaceBillingReadiness::Match => {
            "Card identity matches the current patient and coverage records."
        }
        WorkspaceBillingReadiness::Mismatch => {
            "Printed card identity does not match; clinical chart saves remain available."
        }
        WorkspaceBillingReadiness::Unverified => "No human-confirmed card comparison is on file.",
        WorkspaceBillingReadiness::Stale => {
            "Patient or coverage identity changed after the last card check."
        }
        WorkspaceBillingReadiness::Incomplete => {
            "Complete required patient and coverage identity before card verification."
        }
    }
}

#[cfg(test)]
pub(super) fn billing_export_is_blocked(readiness: WorkspaceBillingReadiness) -> bool {
    readiness != WorkspaceBillingReadiness::Match
}

fn entered_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let value = value.trim();
    if value.is_empty() { fallback } else { value }
}

fn member_id_summary(member_id: &str) -> String {
    let member_id = member_id.trim();
    let characters = member_id.chars().collect::<Vec<_>>();
    if characters.is_empty() {
        return "member not entered".to_string();
    }
    if characters.len() <= 4 {
        return "member entered".to_string();
    }
    let ending = characters[characters.len() - 4..]
        .iter()
        .collect::<String>();
    format!("member ending {ending}")
}

fn option_text(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

fn optional_entry(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[path = "coverage_conversion.rs"]
mod conversion;

#[cfg(test)]
#[path = "coverage_tests.rs"]
mod tests;
