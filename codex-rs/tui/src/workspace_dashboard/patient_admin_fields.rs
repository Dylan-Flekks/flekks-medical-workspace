use super::WorkspaceProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DemographicsField {
    DisplayName,
    PreferredName,
    DateOfBirth,
    SexOrGender,
    ExternalId,
    RecordStartDate,
    RecordEndDate,
    Summary,
}

impl DemographicsField {
    pub(super) const ALL: [Self; 8] = [
        Self::DisplayName,
        Self::PreferredName,
        Self::DateOfBirth,
        Self::SexOrGender,
        Self::ExternalId,
        Self::RecordStartDate,
        Self::RecordEndDate,
        Self::Summary,
    ];

    pub(super) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0)
    }

    pub(super) fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub(super) fn previous(self) -> Self {
        let index = self.index();
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub(super) fn label(self, profile: WorkspaceProfile) -> &'static str {
        match (profile, self) {
            (_, Self::DisplayName) => "Display",
            (_, Self::PreferredName) => "Preferred",
            (_, Self::DateOfBirth) => "DOB",
            (_, Self::SexOrGender) => "Sex/Gender",
            (WorkspaceProfile::Medical, Self::ExternalId) => "Patient ID / MRN",
            (_, Self::ExternalId) => "External ID",
            (WorkspaceProfile::Medical, Self::RecordStartDate) => "Chart start",
            (WorkspaceProfile::Medical, Self::RecordEndDate) => "Chart end",
            (_, Self::RecordStartDate) => "Record start",
            (_, Self::RecordEndDate) => "Record end",
            (_, Self::Summary) => "Summary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PatientAdminEditMode {
    Contact,
    Coverage,
}

impl PatientAdminEditMode {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Contact => "Patient Demographics Editor",
            Self::Coverage => "Coverage Editor",
        }
    }

    pub(super) fn fields(self) -> &'static [PatientAdminField] {
        match self {
            Self::Contact => &CONTACT_ADMIN_FIELDS,
            Self::Coverage => &COVERAGE_ADMIN_FIELDS,
        }
    }

    pub(super) fn first_field(self) -> PatientAdminField {
        self.fields()
            .first()
            .copied()
            .unwrap_or(PatientAdminField::PrimaryPhone)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PatientAdminField {
    DisplayName,
    PreferredName,
    LegalFirstName,
    LegalMiddleName,
    LegalLastName,
    LegalSuffix,
    PreviousName,
    DateOfBirth,
    AdministrativeSex,
    SexOrGender,
    PatientId,
    PreferredLanguage,
    InterpreterRequired,
    PrimaryPhone,
    PrimaryPhoneUse,
    SecondaryPhone,
    SecondaryPhoneUse,
    Email,
    SecondaryEmail,
    PreferredContactMethod,
    AddressLine1,
    AddressLine2,
    City,
    StateOrProvince,
    PostalCode,
    Country,
    AddressUse,
    EmergencyContactName,
    EmergencyContactRelationship,
    EmergencyContactPhone,
    EmergencyContactEmail,
    ContactNotes,
    PayerName,
    PlanName,
    MemberId,
    GroupNumber,
    CoverageType,
    CoverageStatus,
    CoverageNotes,
}

const CONTACT_ADMIN_FIELDS: [PatientAdminField; 32] = [
    PatientAdminField::DisplayName,
    PatientAdminField::PreferredName,
    PatientAdminField::LegalFirstName,
    PatientAdminField::LegalMiddleName,
    PatientAdminField::LegalLastName,
    PatientAdminField::LegalSuffix,
    PatientAdminField::PreviousName,
    PatientAdminField::DateOfBirth,
    PatientAdminField::AdministrativeSex,
    PatientAdminField::SexOrGender,
    PatientAdminField::PatientId,
    PatientAdminField::PreferredLanguage,
    PatientAdminField::InterpreterRequired,
    PatientAdminField::PrimaryPhone,
    PatientAdminField::PrimaryPhoneUse,
    PatientAdminField::SecondaryPhone,
    PatientAdminField::SecondaryPhoneUse,
    PatientAdminField::Email,
    PatientAdminField::SecondaryEmail,
    PatientAdminField::PreferredContactMethod,
    PatientAdminField::AddressLine1,
    PatientAdminField::AddressLine2,
    PatientAdminField::City,
    PatientAdminField::StateOrProvince,
    PatientAdminField::PostalCode,
    PatientAdminField::Country,
    PatientAdminField::AddressUse,
    PatientAdminField::EmergencyContactName,
    PatientAdminField::EmergencyContactRelationship,
    PatientAdminField::EmergencyContactPhone,
    PatientAdminField::EmergencyContactEmail,
    PatientAdminField::ContactNotes,
];

const COVERAGE_ADMIN_FIELDS: [PatientAdminField; 7] = [
    PatientAdminField::PayerName,
    PatientAdminField::PlanName,
    PatientAdminField::MemberId,
    PatientAdminField::GroupNumber,
    PatientAdminField::CoverageType,
    PatientAdminField::CoverageStatus,
    PatientAdminField::CoverageNotes,
];

impl PatientAdminField {
    pub(super) fn mode(self) -> PatientAdminEditMode {
        match self {
            Self::DisplayName
            | Self::PreferredName
            | Self::LegalFirstName
            | Self::LegalMiddleName
            | Self::LegalLastName
            | Self::LegalSuffix
            | Self::PreviousName
            | Self::DateOfBirth
            | Self::AdministrativeSex
            | Self::SexOrGender
            | Self::PatientId
            | Self::PreferredLanguage
            | Self::InterpreterRequired
            | Self::PrimaryPhone
            | Self::PrimaryPhoneUse
            | Self::SecondaryPhone
            | Self::SecondaryPhoneUse
            | Self::Email
            | Self::SecondaryEmail
            | Self::PreferredContactMethod
            | Self::AddressLine1
            | Self::AddressLine2
            | Self::City
            | Self::StateOrProvince
            | Self::PostalCode
            | Self::Country
            | Self::AddressUse
            | Self::EmergencyContactName
            | Self::EmergencyContactRelationship
            | Self::EmergencyContactPhone
            | Self::EmergencyContactEmail
            | Self::ContactNotes => PatientAdminEditMode::Contact,
            Self::PayerName
            | Self::PlanName
            | Self::MemberId
            | Self::GroupNumber
            | Self::CoverageType
            | Self::CoverageStatus
            | Self::CoverageNotes => PatientAdminEditMode::Coverage,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::DisplayName => "Display name",
            Self::PreferredName => "Preferred name",
            Self::LegalFirstName => "Legal first",
            Self::LegalMiddleName => "Legal middle",
            Self::LegalLastName => "Legal last",
            Self::LegalSuffix => "Legal suffix",
            Self::PreviousName => "Previous / alias",
            Self::DateOfBirth => "Date of birth",
            Self::AdministrativeSex => "Administrative sex",
            Self::SexOrGender => "Sex / gender note",
            Self::PatientId => "Patient ID / MRN",
            Self::PreferredLanguage => "Preferred language",
            Self::InterpreterRequired => "Interpreter needed",
            Self::PrimaryPhone => "Primary phone",
            Self::PrimaryPhoneUse => "Primary phone type",
            Self::SecondaryPhone => "Secondary phone",
            Self::SecondaryPhoneUse => "Secondary phone type",
            Self::Email => "Primary email",
            Self::SecondaryEmail => "Secondary email",
            Self::PreferredContactMethod => "Preferred contact",
            Self::AddressLine1 => "Address line 1",
            Self::AddressLine2 => "Address line 2",
            Self::City => "City",
            Self::StateOrProvince => "State / province",
            Self::PostalCode => "Postal code",
            Self::Country => "Country",
            Self::AddressUse => "Address use",
            Self::EmergencyContactName => "Emergency name",
            Self::EmergencyContactRelationship => "Emergency relation",
            Self::EmergencyContactPhone => "Emergency phone",
            Self::EmergencyContactEmail => "Emergency email",
            Self::ContactNotes => "Contact notes",
            Self::PayerName => "Payer",
            Self::PlanName => "Plan name",
            Self::MemberId => "Member ID / Medicare ID",
            Self::GroupNumber => "Group number",
            Self::CoverageType => "Coverage type",
            Self::CoverageStatus => "Coverage status",
            Self::CoverageNotes => "Coverage notes",
        }
    }

    pub(super) fn next_in(self, mode: PatientAdminEditMode) -> Self {
        let fields = mode.fields();
        let index = fields.iter().position(|field| *field == self).unwrap_or(0);
        fields[(index + 1) % fields.len()]
    }

    pub(super) fn previous_in(self, mode: PatientAdminEditMode) -> Self {
        let fields = mode.fields();
        let index = fields.iter().position(|field| *field == self).unwrap_or(0);
        fields[(index + fields.len() - 1) % fields.len()]
    }

    pub(super) fn placeholder(self) -> &'static str {
        match self.mode() {
            PatientAdminEditMode::Contact => "missing",
            PatientAdminEditMode::Coverage => "not set",
        }
    }
}
