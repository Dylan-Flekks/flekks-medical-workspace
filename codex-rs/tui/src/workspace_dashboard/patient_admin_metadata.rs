use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PatientAdminMetadata {
    display_name: String,
    preferred_name: String,
    legal_first_name: String,
    legal_middle_name: String,
    legal_last_name: String,
    legal_suffix: String,
    previous_name: String,
    date_of_birth: String,
    administrative_sex: String,
    sex_or_gender: String,
    patient_id: String,
    preferred_language: String,
    interpreter_required: String,
    primary_phone: String,
    primary_phone_use: String,
    secondary_phone: String,
    secondary_phone_use: String,
    email: String,
    secondary_email: String,
    preferred_contact_method: String,
    address_line_1: String,
    address_line_2: String,
    city: String,
    state_or_province: String,
    postal_code: String,
    country: String,
    address_use: String,
    emergency_contact_name: String,
    emergency_contact_relationship: String,
    emergency_contact_phone: String,
    emergency_contact_email: String,
    contact_notes: String,
    payer_name: String,
    plan_name: String,
    member_id: String,
    group_number: String,
    coverage_type: String,
    coverage_status: String,
    coverage_notes: String,
}

impl PatientAdminMetadata {
    fn from_client(client: &WorkspaceClient) -> Self {
        Self {
            display_name: client.display_name.clone(),
            preferred_name: client.preferred_name.clone().unwrap_or_default(),
            legal_first_name: client.legal_first_name.clone().unwrap_or_default(),
            legal_middle_name: client.legal_middle_name.clone().unwrap_or_default(),
            legal_last_name: client.legal_last_name.clone().unwrap_or_default(),
            legal_suffix: client.legal_suffix.clone().unwrap_or_default(),
            previous_name: client.previous_name.clone().unwrap_or_default(),
            date_of_birth: client.date_of_birth.clone().unwrap_or_default(),
            administrative_sex: client.administrative_sex.clone().unwrap_or_default(),
            sex_or_gender: client.sex_or_gender.clone().unwrap_or_default(),
            patient_id: client.external_id.clone().unwrap_or_default(),
            preferred_language: client.preferred_language.clone().unwrap_or_default(),
            interpreter_required: if client.interpreter_required {
                "yes".to_string()
            } else {
                "no".to_string()
            },
            primary_phone: client.primary_phone.clone().unwrap_or_default(),
            primary_phone_use: client.primary_phone_use.clone().unwrap_or_default(),
            secondary_phone: client.secondary_phone.clone().unwrap_or_default(),
            secondary_phone_use: client.secondary_phone_use.clone().unwrap_or_default(),
            email: client
                .primary_email
                .clone()
                .or_else(|| client.email.clone())
                .unwrap_or_default(),
            secondary_email: client.secondary_email.clone().unwrap_or_default(),
            preferred_contact_method: client.preferred_contact_method.clone().unwrap_or_default(),
            address_line_1: client.address_line_1.clone().unwrap_or_default(),
            address_line_2: client.address_line_2.clone().unwrap_or_default(),
            city: client.city.clone().unwrap_or_default(),
            state_or_province: client.state_or_province.clone().unwrap_or_default(),
            postal_code: client.postal_code.clone().unwrap_or_default(),
            country: client.country.clone().unwrap_or_default(),
            address_use: client.address_use.clone().unwrap_or_default(),
            emergency_contact_name: client.emergency_contact_name.clone().unwrap_or_default(),
            emergency_contact_relationship: client
                .emergency_contact_relationship
                .clone()
                .unwrap_or_default(),
            emergency_contact_phone: client.emergency_contact_phone.clone().unwrap_or_default(),
            emergency_contact_email: client.emergency_contact_email.clone().unwrap_or_default(),
            contact_notes: client.contact_notes.clone().unwrap_or_default(),
            payer_name: client.payer_name.clone().unwrap_or_default(),
            plan_name: client.plan_name.clone().unwrap_or_default(),
            member_id: client.member_id.clone().unwrap_or_default(),
            group_number: client.group_number.clone().unwrap_or_default(),
            coverage_type: client.coverage_type.clone().unwrap_or_default(),
            coverage_status: client.coverage_status.clone().unwrap_or_default(),
            coverage_notes: client.coverage_notes.clone().unwrap_or_default(),
        }
    }

    fn from_draft(draft: &ClientDraft) -> Self {
        Self {
            display_name: draft.display_name.clone(),
            preferred_name: draft.preferred_name.clone(),
            legal_first_name: draft.legal_first_name.clone(),
            legal_middle_name: draft.legal_middle_name.clone(),
            legal_last_name: draft.legal_last_name.clone(),
            legal_suffix: draft.legal_suffix.clone(),
            previous_name: draft.previous_name.clone(),
            date_of_birth: draft.date_of_birth.clone(),
            administrative_sex: draft.administrative_sex.clone(),
            sex_or_gender: draft.sex_or_gender.clone(),
            patient_id: draft.external_id.clone(),
            preferred_language: draft.preferred_language.clone(),
            interpreter_required: draft.interpreter_required.clone(),
            primary_phone: draft.primary_phone.clone(),
            primary_phone_use: draft.primary_phone_use.clone(),
            secondary_phone: draft.secondary_phone.clone(),
            secondary_phone_use: draft.secondary_phone_use.clone(),
            email: draft.email.clone(),
            secondary_email: draft.secondary_email.clone(),
            preferred_contact_method: draft.preferred_contact_method.clone(),
            address_line_1: draft.address_line_1.clone(),
            address_line_2: draft.address_line_2.clone(),
            city: draft.city.clone(),
            state_or_province: draft.state_or_province.clone(),
            postal_code: draft.postal_code.clone(),
            country: draft.country.clone(),
            address_use: draft.address_use.clone(),
            emergency_contact_name: draft.emergency_contact_name.clone(),
            emergency_contact_relationship: draft.emergency_contact_relationship.clone(),
            emergency_contact_phone: draft.emergency_contact_phone.clone(),
            emergency_contact_email: draft.emergency_contact_email.clone(),
            contact_notes: draft.contact_notes.clone(),
            payer_name: draft.payer_name.clone(),
            plan_name: draft.plan_name.clone(),
            member_id: draft.member_id.clone(),
            group_number: draft.group_number.clone(),
            coverage_type: draft.coverage_type.clone(),
            coverage_status: draft.coverage_status.clone(),
            coverage_notes: draft.coverage_notes.clone(),
        }
    }

    pub(super) fn has_contact(&self) -> bool {
        [
            self.primary_phone.as_str(),
            self.secondary_phone.as_str(),
            self.email.as_str(),
        ]
        .iter()
        .any(|value| !value.trim().is_empty())
    }

    pub(super) fn has_emergency_contact(&self) -> bool {
        [
            self.emergency_contact_name.as_str(),
            self.emergency_contact_phone.as_str(),
            self.emergency_contact_email.as_str(),
        ]
        .iter()
        .any(|value| !value.trim().is_empty())
    }

    pub(super) fn has_coverage(&self) -> bool {
        [
            self.payer_name.as_str(),
            self.plan_name.as_str(),
            self.member_id.as_str(),
            self.group_number.as_str(),
            self.coverage_type.as_str(),
            self.coverage_status.as_str(),
        ]
        .iter()
        .any(|value| !value.trim().is_empty())
    }

    pub(super) fn contact_status_label(&self) -> &'static str {
        if self.has_contact() {
            "Contact on file"
        } else {
            "Missing contact"
        }
    }

    pub(super) fn emergency_status_label(&self) -> &'static str {
        if self.has_emergency_contact() {
            "emergency present"
        } else {
            "emergency missing"
        }
    }

    pub(super) fn coverage_status_label(&self) -> &'static str {
        if self.has_coverage() {
            "Coverage on file"
        } else {
            "Missing coverage"
        }
    }

    pub(super) fn contact_summary(&self) -> String {
        let phone = nonempty_or(&self.primary_phone, "phone missing");
        let email = nonempty_or(&self.email, "email missing");
        let method = nonempty_or(&self.preferred_contact_method, "method not set");
        format!("{phone}; {email}; {method}")
    }

    pub(super) fn legal_name_summary(&self) -> String {
        let name = [
            self.legal_first_name.trim(),
            self.legal_middle_name.trim(),
            self.legal_last_name.trim(),
            self.legal_suffix.trim(),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        if name.is_empty() {
            "legal name missing".to_string()
        } else {
            name
        }
    }

    pub(super) fn address_summary(&self) -> String {
        let locality = [
            self.city.trim(),
            self.state_or_province.trim(),
            self.postal_code.trim(),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        let line = nonempty_or(&self.address_line_1, "address missing");
        if locality.is_empty() {
            line
        } else {
            format!("{line}; {locality}")
        }
    }

    pub(super) fn emergency_summary(&self) -> String {
        let name = nonempty_or(&self.emergency_contact_name, "name missing");
        let relationship =
            nonempty_or(&self.emergency_contact_relationship, "relationship missing");
        let phone = nonempty_or(&self.emergency_contact_phone, "phone missing");
        format!("{name}; {relationship}; {phone}")
    }

    pub(super) fn search_values(&self) -> Vec<String> {
        [
            &self.display_name,
            &self.preferred_name,
            &self.legal_first_name,
            &self.legal_middle_name,
            &self.legal_last_name,
            &self.legal_suffix,
            &self.previous_name,
            &self.date_of_birth,
            &self.administrative_sex,
            &self.sex_or_gender,
            &self.patient_id,
            &self.preferred_language,
            &self.interpreter_required,
            &self.primary_phone,
            &self.primary_phone_use,
            &self.secondary_phone,
            &self.secondary_phone_use,
            &self.email,
            &self.secondary_email,
            &self.preferred_contact_method,
            &self.address_line_1,
            &self.address_line_2,
            &self.city,
            &self.state_or_province,
            &self.postal_code,
            &self.country,
            &self.address_use,
            &self.emergency_contact_name,
            &self.emergency_contact_relationship,
            &self.emergency_contact_phone,
            &self.emergency_contact_email,
            &self.contact_notes,
            &self.payer_name,
            &self.plan_name,
            &self.member_id,
            &self.group_number,
            &self.coverage_type,
            &self.coverage_status,
            &self.coverage_notes,
        ]
        .iter()
        .filter_map(|value| nonempty_option(value))
        .collect()
    }

    pub(super) fn value(&self, field: PatientAdminField) -> &str {
        match field {
            PatientAdminField::DisplayName => &self.display_name,
            PatientAdminField::PreferredName => &self.preferred_name,
            PatientAdminField::LegalFirstName => &self.legal_first_name,
            PatientAdminField::LegalMiddleName => &self.legal_middle_name,
            PatientAdminField::LegalLastName => &self.legal_last_name,
            PatientAdminField::LegalSuffix => &self.legal_suffix,
            PatientAdminField::PreviousName => &self.previous_name,
            PatientAdminField::DateOfBirth => &self.date_of_birth,
            PatientAdminField::AdministrativeSex => &self.administrative_sex,
            PatientAdminField::SexOrGender => &self.sex_or_gender,
            PatientAdminField::PatientId => &self.patient_id,
            PatientAdminField::PreferredLanguage => &self.preferred_language,
            PatientAdminField::InterpreterRequired => &self.interpreter_required,
            PatientAdminField::PrimaryPhone => &self.primary_phone,
            PatientAdminField::PrimaryPhoneUse => &self.primary_phone_use,
            PatientAdminField::SecondaryPhone => &self.secondary_phone,
            PatientAdminField::SecondaryPhoneUse => &self.secondary_phone_use,
            PatientAdminField::Email => &self.email,
            PatientAdminField::SecondaryEmail => &self.secondary_email,
            PatientAdminField::PreferredContactMethod => &self.preferred_contact_method,
            PatientAdminField::AddressLine1 => &self.address_line_1,
            PatientAdminField::AddressLine2 => &self.address_line_2,
            PatientAdminField::City => &self.city,
            PatientAdminField::StateOrProvince => &self.state_or_province,
            PatientAdminField::PostalCode => &self.postal_code,
            PatientAdminField::Country => &self.country,
            PatientAdminField::AddressUse => &self.address_use,
            PatientAdminField::EmergencyContactName => &self.emergency_contact_name,
            PatientAdminField::EmergencyContactRelationship => &self.emergency_contact_relationship,
            PatientAdminField::EmergencyContactPhone => &self.emergency_contact_phone,
            PatientAdminField::EmergencyContactEmail => &self.emergency_contact_email,
            PatientAdminField::ContactNotes => &self.contact_notes,
            PatientAdminField::PayerName => &self.payer_name,
            PatientAdminField::PlanName => &self.plan_name,
            PatientAdminField::MemberId => &self.member_id,
            PatientAdminField::GroupNumber => &self.group_number,
            PatientAdminField::CoverageType => &self.coverage_type,
            PatientAdminField::CoverageStatus => &self.coverage_status,
            PatientAdminField::CoverageNotes => &self.coverage_notes,
        }
    }
}

pub(super) fn patient_admin_metadata_for_client(client: &WorkspaceClient) -> PatientAdminMetadata {
    PatientAdminMetadata::from_client(client)
}

pub(super) fn patient_admin_metadata_for_draft(draft: &ClientDraft) -> PatientAdminMetadata {
    PatientAdminMetadata::from_draft(draft)
}
