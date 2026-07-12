use super::CoverageDraft;
use super::CoveragePriority;
use super::InvalidCoveragePriority;
use super::option_text;
use super::optional_entry;
use codex_app_server_protocol::WorkspaceCoverage;
use codex_app_server_protocol::WorkspaceCoverageUpsertParams;
use std::fmt;

impl fmt::Display for InvalidCoveragePriority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "coverage priority must be 1, 2, or 3; received {}",
            self.0
        )
    }
}

impl std::error::Error for InvalidCoveragePriority {}

impl TryFrom<i64> for CoveragePriority {
    type Error = InvalidCoveragePriority;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Primary),
            2 => Ok(Self::Secondary),
            3 => Ok(Self::Tertiary),
            invalid => Err(InvalidCoveragePriority(invalid)),
        }
    }
}

impl TryFrom<&WorkspaceCoverage> for CoverageDraft {
    type Error = InvalidCoveragePriority;

    fn try_from(value: &WorkspaceCoverage) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Some(value.id.clone()),
            version: Some(value.version.clone()),
            client_id: value.client_id.clone(),
            priority: value.priority.try_into()?,
            payer_name: option_text(&value.payer_name),
            plan_name: option_text(&value.plan_name),
            member_id: option_text(&value.member_id),
            group_number: option_text(&value.group_number),
            coverage_type: option_text(&value.coverage_type),
            coverage_status: option_text(&value.coverage_status),
            effective_date: option_text(&value.effective_date),
            termination_date: option_text(&value.termination_date),
            patient_relationship_to_subscriber: option_text(
                &value.patient_relationship_to_subscriber,
            ),
            subscriber_first_name: option_text(&value.subscriber_first_name),
            subscriber_middle_name: option_text(&value.subscriber_middle_name),
            subscriber_last_name: option_text(&value.subscriber_last_name),
            subscriber_suffix: option_text(&value.subscriber_suffix),
            subscriber_date_of_birth: option_text(&value.subscriber_date_of_birth),
            subscriber_administrative_sex: option_text(&value.subscriber_administrative_sex),
            subscriber_address_same_as_patient: value.subscriber_address_same_as_patient,
            subscriber_address_line_1: option_text(&value.subscriber_address_line_1),
            subscriber_address_line_2: option_text(&value.subscriber_address_line_2),
            subscriber_city: option_text(&value.subscriber_city),
            subscriber_state_or_province: option_text(&value.subscriber_state_or_province),
            subscriber_postal_code: option_text(&value.subscriber_postal_code),
            subscriber_country: option_text(&value.subscriber_country),
            coverage_notes: option_text(&value.coverage_notes),
            billing_readiness: value.billing_readiness,
        })
    }
}

impl TryFrom<WorkspaceCoverage> for CoverageDraft {
    type Error = InvalidCoveragePriority;

    fn try_from(value: WorkspaceCoverage) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<&WorkspaceCoverageUpsertParams> for CoverageDraft {
    type Error = InvalidCoveragePriority;

    fn try_from(value: &WorkspaceCoverageUpsertParams) -> Result<Self, Self::Error> {
        let mut draft = Self::new(value.client_id.clone(), value.priority.try_into()?);
        draft.id.clone_from(&value.id);
        draft.payer_name = option_text(&value.payer_name);
        draft.plan_name = option_text(&value.plan_name);
        draft.member_id = option_text(&value.member_id);
        draft.group_number = option_text(&value.group_number);
        draft.coverage_type = option_text(&value.coverage_type);
        draft.coverage_status = option_text(&value.coverage_status);
        draft.effective_date = option_text(&value.effective_date);
        draft.termination_date = option_text(&value.termination_date);
        draft.patient_relationship_to_subscriber =
            option_text(&value.patient_relationship_to_subscriber);
        draft.subscriber_first_name = option_text(&value.subscriber_first_name);
        draft.subscriber_middle_name = option_text(&value.subscriber_middle_name);
        draft.subscriber_last_name = option_text(&value.subscriber_last_name);
        draft.subscriber_suffix = option_text(&value.subscriber_suffix);
        draft.subscriber_date_of_birth = option_text(&value.subscriber_date_of_birth);
        draft.subscriber_administrative_sex = option_text(&value.subscriber_administrative_sex);
        draft.subscriber_address_same_as_patient = value.subscriber_address_same_as_patient;
        draft.subscriber_address_line_1 = option_text(&value.subscriber_address_line_1);
        draft.subscriber_address_line_2 = option_text(&value.subscriber_address_line_2);
        draft.subscriber_city = option_text(&value.subscriber_city);
        draft.subscriber_state_or_province = option_text(&value.subscriber_state_or_province);
        draft.subscriber_postal_code = option_text(&value.subscriber_postal_code);
        draft.subscriber_country = option_text(&value.subscriber_country);
        draft.coverage_notes = option_text(&value.coverage_notes);
        Ok(draft)
    }
}

impl TryFrom<WorkspaceCoverageUpsertParams> for CoverageDraft {
    type Error = InvalidCoveragePriority;

    fn try_from(value: WorkspaceCoverageUpsertParams) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl From<&CoverageDraft> for WorkspaceCoverageUpsertParams {
    fn from(value: &CoverageDraft) -> Self {
        Self {
            id: value.id.clone(),
            client_id: value.client_id.trim().to_string(),
            priority: value.priority.number(),
            payer_name: optional_entry(&value.payer_name),
            plan_name: optional_entry(&value.plan_name),
            member_id: optional_entry(&value.member_id),
            group_number: optional_entry(&value.group_number),
            coverage_type: optional_entry(&value.coverage_type),
            coverage_status: optional_entry(&value.coverage_status),
            effective_date: optional_entry(&value.effective_date),
            termination_date: optional_entry(&value.termination_date),
            patient_relationship_to_subscriber: optional_entry(
                &value.patient_relationship_to_subscriber,
            ),
            subscriber_first_name: optional_entry(&value.subscriber_first_name),
            subscriber_middle_name: optional_entry(&value.subscriber_middle_name),
            subscriber_last_name: optional_entry(&value.subscriber_last_name),
            subscriber_suffix: optional_entry(&value.subscriber_suffix),
            subscriber_date_of_birth: optional_entry(&value.subscriber_date_of_birth),
            subscriber_administrative_sex: optional_entry(&value.subscriber_administrative_sex),
            subscriber_address_same_as_patient: value.subscriber_address_same_as_patient,
            subscriber_address_line_1: optional_entry(&value.subscriber_address_line_1),
            subscriber_address_line_2: optional_entry(&value.subscriber_address_line_2),
            subscriber_city: optional_entry(&value.subscriber_city),
            subscriber_state_or_province: optional_entry(&value.subscriber_state_or_province),
            subscriber_postal_code: optional_entry(&value.subscriber_postal_code),
            subscriber_country: optional_entry(&value.subscriber_country),
            coverage_notes: optional_entry(&value.coverage_notes),
        }
    }
}

impl From<CoverageDraft> for WorkspaceCoverageUpsertParams {
    fn from(value: CoverageDraft) -> Self {
        Self::from(&value)
    }
}
