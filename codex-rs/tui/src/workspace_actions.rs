use crate::workspace_dashboard::WorkspaceProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceActionId {
    Actions,
    AddendumSave,
    AddendumStart,
    AgentClear,
    AgentInbox,
    AgentPacketInspect,
    AgentPreview,
    AgentRequest,
    AgentResult,
    AgentResultClear,
    AgentResultDismiss,
    AgentResultInspect,
    AgentResultNext,
    AgentResultReviewed,
    AgentResultSave,
    AgentResultToAddendum,
    AgentResultToJob,
    AgentResultToProposal,
    ArtifactClear,
    ArtifactDeselect,
    ArtifactInspect,
    ArtifactImport,
    ArtifactOpen,
    ArtifactSave,
    ArtifactSelect,
    ArtifactThumbnail,
    ArtifactScope,
    ArtifactToggle,
    ClientNew,
    ClipArchive,
    ClipClear,
    ClipDeselect,
    ClipInspect,
    ClipNew,
    ClipReviewed,
    ClipSave,
    ClipSelect,
    ClipToggle,
    ContactEdit,
    DerivativeArchive,
    DerivativeClear,
    DerivativeDeselect,
    DerivativeInspect,
    DerivativeNew,
    DerivativeReviewed,
    DerivativeSave,
    DerivativeSelect,
    DerivativeToggle,
    DiscardLocalDraft,
    DocumentAttach,
    DocumentChoose,
    EncounterOpen,
    EmergencyContactEdit,
    FocusAddenda,
    FocusDocuments,
    FocusJobs,
    FocusNoteBody,
    FocusNoteTitle,
    FocusPatientDetails,
    FocusPatients,
    PatientSearch,
    CoverageEdit,
    CoverageVerify,
    FocusProposals,
    FocusTimeline,
    FocusVisit,
    FocusWorkflow,
    FocusWorkflowAudit,
    FocusWorkflowNoteStatus,
    Handoff,
    JobCancel,
    JobDone,
    JobNew,
    JobNext,
    NoteNew,
    NoteSign,
    PracticeLibraryAssociate,
    PracticeLibraryInspect,
    PracticeLibraryNext,
    ProposalAccept,
    ProposalDecline,
    ProposalNext,
    RestoreLocalDraft,
    Return,
    Save,
    SafetyOpen,
    AllergyAdd,
    MedicationAdd,
    ProblemAdd,
    PrecautionAdd,
    ScopeAgentPacket,
    ScopeAgentReview,
    ScopeAuditTrail,
    ScopeLast,
    ScopeNext,
    ScopePatientChart,
    ScopePatientFiles,
    ScopePrevious,
    ScopePracticeIntelligence,
    ScopePracticeLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceActionGroup {
    Addenda,
    AgentContext,
    AgentHandoff,
    ClinicalSafety,
    Documents,
    Encounters,
    Jobs,
    Navigation,
    Notes,
    Proposals,
    Records,
    SaveReturn,
    Search,
    Scopes,
    WorkflowSections,
}

impl WorkspaceActionGroup {
    pub(crate) const ALL: [Self; 15] = [
        Self::Navigation,
        Self::Records,
        Self::Search,
        Self::ClinicalSafety,
        Self::Notes,
        Self::Encounters,
        Self::Documents,
        Self::Scopes,
        Self::AgentContext,
        Self::Proposals,
        Self::Addenda,
        Self::Jobs,
        Self::WorkflowSections,
        Self::AgentHandoff,
        Self::SaveReturn,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Addenda => "Addenda",
            Self::AgentContext => "Medical Agent Plan and returned work",
            Self::AgentHandoff => "Medical Agent Plan",
            Self::ClinicalSafety => "Clinical safety",
            Self::Documents => "Files & reviewed text",
            Self::Encounters => "Encounters",
            Self::Jobs => "Jobs",
            Self::Navigation => "Navigation",
            Self::Notes => "Notes",
            Self::Proposals => "Note proposals",
            Self::Records => "Records / Patients",
            Self::SaveReturn => "Save and return",
            Self::Search => "Search",
            Self::Scopes => "Clinical scopes",
            Self::WorkflowSections => "Workflow sections",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceActionProfile {
    Both,
    Generic,
    Medical,
}

impl WorkspaceActionProfile {
    fn matches(self, profile: WorkspaceProfile) -> bool {
        match self {
            Self::Both => true,
            Self::Generic => profile == WorkspaceProfile::Generic,
            Self::Medical => profile == WorkspaceProfile::Medical,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkspaceActionDef {
    pub(crate) id: WorkspaceActionId,
    pub(crate) label: &'static str,
    pub(crate) command: &'static str,
    pub(crate) shortcut: Option<&'static str>,
    pub(crate) group: WorkspaceActionGroup,
    profile: WorkspaceActionProfile,
}

impl WorkspaceActionDef {
    pub(crate) fn applies_to(self, profile: WorkspaceProfile) -> bool {
        self.profile.matches(profile)
    }
}

pub(crate) const WORKSPACE_ACTIONS: &[WorkspaceActionDef] = &[
    WorkspaceActionDef {
        id: WorkspaceActionId::Actions,
        label: "Show workspace actions",
        command: "actions",
        shortcut: Some("?"),
        group: WorkspaceActionGroup::Navigation,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusPatients,
        label: "Open patient list",
        command: "patients",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::PatientSearch,
        label: "Search patients",
        command: "patient search",
        shortcut: Some("/"),
        group: WorkspaceActionGroup::Search,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusPatientDetails,
        label: "Open patient demographics",
        command: "demographics",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ContactEdit,
        label: "Edit patient demographics",
        command: "demographics edit",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::EmergencyContactEdit,
        label: "Edit emergency contact",
        command: "emergency contact edit",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::CoverageEdit,
        label: "Edit coverage",
        command: "coverage edit",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::CoverageVerify,
        label: "Compare coverage card",
        command: "coverage verify",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::RestoreLocalDraft,
        label: "Restore local draft",
        command: "draft restore",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DiscardLocalDraft,
        label: "Discard local draft",
        command: "draft discard",
        shortcut: None,
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::SafetyOpen,
        label: "Open clinical safety",
        command: "safety",
        shortcut: None,
        group: WorkspaceActionGroup::ClinicalSafety,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AllergyAdd,
        label: "Add allergy",
        command: "allergy add",
        shortcut: None,
        group: WorkspaceActionGroup::ClinicalSafety,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::MedicationAdd,
        label: "Add medication",
        command: "medication add",
        shortcut: None,
        group: WorkspaceActionGroup::ClinicalSafety,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ProblemAdd,
        label: "Add problem",
        command: "problem add",
        shortcut: None,
        group: WorkspaceActionGroup::ClinicalSafety,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::PrecautionAdd,
        label: "Add precaution",
        command: "precaution add",
        shortcut: None,
        group: WorkspaceActionGroup::ClinicalSafety,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusNoteTitle,
        label: "Focus note title",
        command: "focus note title",
        shortcut: None,
        group: WorkspaceActionGroup::Navigation,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusNoteBody,
        label: "Focus note body",
        command: "focus note body",
        shortcut: None,
        group: WorkspaceActionGroup::Navigation,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusWorkflow,
        label: "Focus clinical workspace",
        command: "focus clinical workspace",
        shortcut: None,
        group: WorkspaceActionGroup::Navigation,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopePatientChart,
        label: "Open Patient Chart",
        command: "scope patient",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopePatientFiles,
        label: "Open Patient File Tree",
        command: "scope files",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopeAgentPacket,
        label: "Open Medical Agent Plan",
        command: "agent handoff",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopeAgentReview,
        label: "Open returned work review",
        command: "scope review",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopePracticeLibrary,
        label: "Open Practice Library",
        command: "scope practice",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::PracticeLibraryNext,
        label: "Select next practice record",
        command: "practice next",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::PracticeLibraryInspect,
        label: "Inspect practice record",
        command: "practice inspect",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::PracticeLibraryAssociate,
        label: "Associate practice file to patient",
        command: "practice associate",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopePracticeIntelligence,
        label: "Open Practice Intelligence",
        command: "scope intelligence",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopeAuditTrail,
        label: "Open Audit Trail",
        command: "scope audit",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopeNext,
        label: "Open next scope",
        command: "scope next",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopePrevious,
        label: "Previous scope",
        command: "scope prev",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ScopeLast,
        label: "Open last scope",
        command: "scope last",
        shortcut: None,
        group: WorkspaceActionGroup::Scopes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusVisit,
        label: "Jump to visit",
        command: "workflow visit",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusWorkflowNoteStatus,
        label: "Jump to note status",
        command: "workflow note status",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusProposals,
        label: "Jump to proposals",
        command: "workflow proposals",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusAddenda,
        label: "Jump to addenda",
        command: "workflow addenda",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusDocuments,
        label: "Jump to documents",
        command: "workflow documents",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusJobs,
        label: "Jump to jobs",
        command: "workflow jobs",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusTimeline,
        label: "Jump to timeline",
        command: "workflow timeline",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusWorkflowAudit,
        label: "Jump to audit",
        command: "workflow audit",
        shortcut: None,
        group: WorkspaceActionGroup::WorkflowSections,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClientNew,
        label: "New client",
        command: "client new",
        shortcut: Some("c"),
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Generic,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClientNew,
        label: "New patient",
        command: "patient new",
        shortcut: Some("c"),
        group: WorkspaceActionGroup::Records,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::NoteNew,
        label: "New note",
        command: "note new",
        shortcut: Some("n"),
        group: WorkspaceActionGroup::Notes,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::EncounterOpen,
        label: "Open encounter",
        command: "encounter open",
        shortcut: Some("e"),
        group: WorkspaceActionGroup::Encounters,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::NoteSign,
        label: "Sign note",
        command: "note sign",
        shortcut: Some("l"),
        group: WorkspaceActionGroup::Notes,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DocumentAttach,
        label: "Add file reference",
        command: "file add",
        shortcut: Some("d"),
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DocumentChoose,
        label: "Choose JPG/PDF from Mac",
        command: "file drop",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactToggle,
        label: "Include/exclude file",
        command: "artifact toggle",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactSelect,
        label: "Include file reference for agent",
        command: "artifact select",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactDeselect,
        label: "Exclude file reference from agent",
        command: "artifact deselect",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactInspect,
        label: "Inspect file reference",
        command: "artifact inspect",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactImport,
        label: "Import to local vault",
        command: "file import",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactThumbnail,
        label: "Generate JPG/PDF preview",
        command: "file thumbnail",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactOpen,
        label: "Open local file",
        command: "file open",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactSave,
        label: "Save file reference",
        command: "artifact save",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactClear,
        label: "Clear file reference draft",
        command: "artifact clear",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ArtifactScope,
        label: "Edit file reference scope",
        command: "artifact scope",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeNew,
        label: "Add reviewed text",
        command: "derivative new",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeSave,
        label: "Save reviewed text",
        command: "derivative save",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeClear,
        label: "Clear reviewed text draft",
        command: "derivative clear",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeToggle,
        label: "Select reviewed text for agent",
        command: "derivative toggle",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeSelect,
        label: "Include reviewed text",
        command: "derivative select",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeDeselect,
        label: "Exclude reviewed text",
        command: "derivative deselect",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeInspect,
        label: "Inspect reviewed text",
        command: "derivative inspect",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeReviewed,
        label: "Mark text reviewed",
        command: "derivative reviewed",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::DerivativeArchive,
        label: "Archive reviewed text",
        command: "derivative archive",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipNew,
        label: "Add context clip",
        command: "clip new",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipSave,
        label: "Save context clip",
        command: "clip save",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipClear,
        label: "Clear clip draft",
        command: "clip clear",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipToggle,
        label: "Select clip for agent",
        command: "clip toggle",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipSelect,
        label: "Include clip excerpt",
        command: "clip select",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipDeselect,
        label: "Exclude clip excerpt",
        command: "clip deselect",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipInspect,
        label: "Inspect context clip",
        command: "clip inspect",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipReviewed,
        label: "Mark clip reviewed",
        command: "clip reviewed",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ClipArchive,
        label: "Archive context clip",
        command: "clip archive",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusDocuments,
        label: "Jump to patient files",
        command: "patient files",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusDocuments,
        label: "Jump to practice files",
        command: "practice files",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::FocusDocuments,
        label: "Jump to EDI files",
        command: "edi files",
        shortcut: None,
        group: WorkspaceActionGroup::Documents,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentRequest,
        label: "Write agent instructions",
        command: "agent request",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentPreview,
        label: "Review Packet",
        command: "agent preview",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentInbox,
        label: "Open returned work review",
        command: "agent inbox",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentPacketInspect,
        label: "Compare medical plan and result",
        command: "agent handoff inspect",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentPacketInspect,
        label: "Compare medical plan and result",
        command: "agent context inspect",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResult,
        label: "Paste returned agent work",
        command: "agent result",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultSave,
        label: "Save returned agent work",
        command: "agent result save",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultClear,
        label: "Clear returned work",
        command: "agent result clear",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultInspect,
        label: "Inspect returned agent work",
        command: "agent result inspect",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultNext,
        label: "Next returned work",
        command: "agent result next",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultReviewed,
        label: "Mark returned work reviewed",
        command: "agent result reviewed",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultDismiss,
        label: "Dismiss returned work",
        command: "agent result dismiss",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentClear,
        label: "Clear agent instructions",
        command: "agent clear",
        shortcut: None,
        group: WorkspaceActionGroup::AgentContext,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::Handoff,
        label: "Submit Medical Agent Plan",
        command: "agent send",
        shortcut: None,
        group: WorkspaceActionGroup::AgentHandoff,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ProposalNext,
        label: "Select next proposal",
        command: "proposal next",
        shortcut: None,
        group: WorkspaceActionGroup::Proposals,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ProposalAccept,
        label: "Accept proposal",
        command: "proposal accept",
        shortcut: None,
        group: WorkspaceActionGroup::Proposals,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::ProposalDecline,
        label: "Decline proposal",
        command: "proposal decline",
        shortcut: None,
        group: WorkspaceActionGroup::Proposals,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultToProposal,
        label: "Make note proposal from returned work",
        command: "agent result to proposal",
        shortcut: None,
        group: WorkspaceActionGroup::Proposals,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AddendumStart,
        label: "Start addendum",
        command: "addendum start",
        shortcut: None,
        group: WorkspaceActionGroup::Addenda,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AddendumSave,
        label: "Save addendum",
        command: "addendum save",
        shortcut: None,
        group: WorkspaceActionGroup::Addenda,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultToAddendum,
        label: "Make addendum from returned work",
        command: "agent result to addendum",
        shortcut: None,
        group: WorkspaceActionGroup::Addenda,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::Handoff,
        label: "Submit Medical Agent Plan",
        command: "handoff",
        shortcut: Some("Ctrl-G"),
        group: WorkspaceActionGroup::AgentHandoff,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::Save,
        label: "Save chart workspace",
        command: "save",
        shortcut: Some("Ctrl-S"),
        group: WorkspaceActionGroup::SaveReturn,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::Return,
        label: "Return to agent",
        command: "return",
        shortcut: Some("Ctrl-W / Esc"),
        group: WorkspaceActionGroup::SaveReturn,
        profile: WorkspaceActionProfile::Both,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::JobNew,
        label: "New job",
        command: "job new",
        shortcut: None,
        group: WorkspaceActionGroup::Jobs,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::JobNext,
        label: "Select next job",
        command: "job next",
        shortcut: None,
        group: WorkspaceActionGroup::Jobs,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::JobDone,
        label: "Complete job",
        command: "job done",
        shortcut: None,
        group: WorkspaceActionGroup::Jobs,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::JobCancel,
        label: "Cancel job",
        command: "job cancel",
        shortcut: None,
        group: WorkspaceActionGroup::Jobs,
        profile: WorkspaceActionProfile::Medical,
    },
    WorkspaceActionDef {
        id: WorkspaceActionId::AgentResultToJob,
        label: "Make job from returned work",
        command: "agent result to job",
        shortcut: None,
        group: WorkspaceActionGroup::Jobs,
        profile: WorkspaceActionProfile::Medical,
    },
];

pub(crate) fn action_for_command(
    profile: WorkspaceProfile,
    command: &str,
) -> Option<WorkspaceActionId> {
    let command = normalize_workspace_command(command);
    if profile == WorkspaceProfile::Medical {
        match command.as_str() {
            "workflow" | "clinical workspace" | "practice workflow" => {
                return Some(WorkspaceActionId::FocusWorkflow);
            }
            "patients" | "patient list" | "patient directory" | "open patient list" => {
                return Some(WorkspaceActionId::FocusPatients);
            }
            "patient search" | "search patients" | "patient lookup" | "patient find" => {
                return Some(WorkspaceActionId::PatientSearch);
            }
            "patient switch" | "switch patient" => {
                return Some(WorkspaceActionId::PatientSearch);
            }
            "demographics" | "patient demographics" | "patient details" | "patient summary" => {
                return Some(WorkspaceActionId::FocusPatientDetails);
            }
            "demographics edit"
            | "patient demographics edit"
            | "contact edit"
            | "edit contact"
            | "edit contact info" => {
                return Some(WorkspaceActionId::ContactEdit);
            }
            "emergency contact edit" | "edit emergency contact" => {
                return Some(WorkspaceActionId::EmergencyContactEdit);
            }
            "coverage edit" | "edit coverage" => return Some(WorkspaceActionId::CoverageEdit),
            "coverage verify"
            | "verify coverage"
            | "compare coverage card"
            | "card compare" => return Some(WorkspaceActionId::CoverageVerify),
            "draft restore" | "restore draft" | "restore local draft" => {
                return Some(WorkspaceActionId::RestoreLocalDraft);
            }
            "draft discard" | "discard draft" | "discard local draft" => {
                return Some(WorkspaceActionId::DiscardLocalDraft);
            }
            "safety" | "clinical safety" | "allergies" | "medications" | "meds" | "problems"
            | "conditions" | "precautions" => return Some(WorkspaceActionId::SafetyOpen),
            "allergy add" | "add allergy" | "new allergy" | "allergies add" => {
                return Some(WorkspaceActionId::AllergyAdd);
            }
            "medication add" | "add medication" | "new medication" | "med add" | "meds add" => {
                return Some(WorkspaceActionId::MedicationAdd);
            }
            "problem add" | "add problem" | "new problem" | "condition add" | "add condition" => {
                return Some(WorkspaceActionId::ProblemAdd);
            }
            "precaution add" | "add precaution" | "new precaution" => {
                return Some(WorkspaceActionId::PrecautionAdd);
            }
            "scope patient"
            | "scope patient chart"
            | "patient chart"
            | "chart"
            | "return to patient chart"
            | "patient chart scope" => return Some(WorkspaceActionId::ScopePatientChart),
            "scope files" | "scope patient files" | "patient files scope" => {
                return Some(WorkspaceActionId::ScopePatientFiles);
            }
            "agent handoff" | "scope packet" | "scope agent packet" | "agent packet"
            | "agent packet scope" => {
                return Some(WorkspaceActionId::ScopeAgentPacket);
            }
            "scope review" | "scope agent review" | "agent review scope" => {
                return Some(WorkspaceActionId::ScopeAgentReview);
            }
            "scope practice"
            | "scope library"
            | "scope practice library"
            | "practice library"
            | "practice library scope" => return Some(WorkspaceActionId::ScopePracticeLibrary),
            "practice next" | "library next" | "practice library next" => {
                return Some(WorkspaceActionId::PracticeLibraryNext);
            }
            "practice inspect" | "library inspect" | "practice library inspect" => {
                return Some(WorkspaceActionId::PracticeLibraryInspect);
            }
            "practice associate"
            | "library associate"
            | "practice library associate"
            | "practice link patient"
            | "library link patient" => return Some(WorkspaceActionId::PracticeLibraryAssociate),
            "scope intelligence"
            | "scope practice intelligence"
            | "practice intelligence"
            | "practice intelligence scope" => {
                return Some(WorkspaceActionId::ScopePracticeIntelligence);
            }
            "scope audit" | "scope audit trail" | "audit trail" | "audit trail scope" => {
                return Some(WorkspaceActionId::ScopeAuditTrail);
            }
            "scope next" | "next scope" | "open next scope" => {
                return Some(WorkspaceActionId::ScopeNext);
            }
            "scope previous" | "scope prev" | "previous scope" => {
                return Some(WorkspaceActionId::ScopePrevious);
            }
            "scope last"
            | "last scope"
            | "open last scope"
            | "scope back"
            | "previous clinical scope" => {
                return Some(WorkspaceActionId::ScopeLast);
            }
            "visit" | "encounter" | "workflow visit" => {
                return Some(WorkspaceActionId::FocusVisit);
            }
            "encounter open" | "encounter create" | "visit open" | "start encounter"
            | "open encounter" | "first eval" | "start eval" | "start first eval" => {
                return Some(WorkspaceActionId::EncounterOpen);
            }
            "note new eval"
            | "note new evaluation"
            | "note new daily"
            | "note new progress"
            | "note new phone"
            | "new evaluation note"
            | "new daily note"
            | "new progress note"
            | "new phone note" => return Some(WorkspaceActionId::NoteNew),
            "note status" | "signing" | "workflow note status" => {
                return Some(WorkspaceActionId::FocusWorkflowNoteStatus);
            }
            "proposals" | "workflow proposals" => return Some(WorkspaceActionId::FocusProposals),
            "addenda" | "focus addenda" | "workflow addenda" => {
                return Some(WorkspaceActionId::FocusAddenda);
            }
            "file add"
            | "file attach"
            | "attach file"
            | "add patient file reference"
            | "document attach"
            | "document add"
            | "attach document" => return Some(WorkspaceActionId::DocumentAttach),
            "file drop"
            | "file choose"
            | "choose file"
            | "choose patient file"
            | "drop file"
            | "drop patient file"
            | "jpg pdf drop"
            | "pdf jpg drop" => return Some(WorkspaceActionId::DocumentChoose),
            "documents"
            | "focus documents"
            | "files"
            | "patient files"
            | "practice files"
            | "file references"
            | "reviewed text"
            | "artifact files"
            | "edi files"
            | "billing files"
            | "add patient file"
            | "patient file intake" => {
                return Some(WorkspaceActionId::FocusDocuments);
            }
            "artifact select"
            | "artifact include"
            | "file reference select"
            | "file reference include" => {
                return Some(WorkspaceActionId::ArtifactSelect);
            }
            "artifact deselect"
            | "artifact exclude"
            | "file reference deselect"
            | "file reference exclude" => {
                return Some(WorkspaceActionId::ArtifactDeselect);
            }
            "artifact save" | "document save" | "file reference save" => {
                return Some(WorkspaceActionId::ArtifactSave);
            }
            "file import"
            | "file vault"
            | "vault file"
            | "import file"
            | "import to vault"
            | "local vault import"
            | "copy to local vault" => return Some(WorkspaceActionId::ArtifactImport),
            "file thumbnail"
            | "file preview"
            | "preview file"
            | "generate preview"
            | "quicklook preview"
            | "quick look preview"
            | "thumbnail"
            | "thumbnail generate"
            | "generate thumbnail"
            | "quicklook thumbnail"
            | "quick look thumbnail" => return Some(WorkspaceActionId::ArtifactThumbnail),
            "file open" | "open file" | "open patient file" | "open local file"
            | "open preview" | "quicklook open" | "quick look open" => {
                return Some(WorkspaceActionId::ArtifactOpen);
            }
            "artifact clear" | "document clear" | "file reference clear" => {
                return Some(WorkspaceActionId::ArtifactClear);
            }
            "artifact scope" | "document scope" | "file reference scope" => {
                return Some(WorkspaceActionId::ArtifactScope);
            }
            "artifact toggle" | "file reference toggle" => {
                return Some(WorkspaceActionId::ArtifactToggle);
            }
            "artifact inspect" | "file reference inspect" => {
                return Some(WorkspaceActionId::ArtifactInspect);
            }
            "derivative new"
            | "reviewed text new"
            | "reviewed text add"
            | "artifact derivative"
            | "transcript add"
            | "ocr add"
            | "video note"
            | "edi summary" => return Some(WorkspaceActionId::DerivativeNew),
            "derivative save" | "reviewed text save" => {
                return Some(WorkspaceActionId::DerivativeSave);
            }
            "derivative clear" | "reviewed text clear" => {
                return Some(WorkspaceActionId::DerivativeClear);
            }
            "derivative select"
            | "derivative include"
            | "reviewed text select"
            | "reviewed text include" => {
                return Some(WorkspaceActionId::DerivativeSelect);
            }
            "derivative deselect"
            | "derivative exclude"
            | "reviewed text deselect"
            | "reviewed text exclude" => {
                return Some(WorkspaceActionId::DerivativeDeselect);
            }
            "derivative toggle" | "reviewed text toggle" => {
                return Some(WorkspaceActionId::DerivativeToggle);
            }
            "derivative inspect" | "reviewed text inspect" => {
                return Some(WorkspaceActionId::DerivativeInspect);
            }
            "derivative reviewed"
            | "derivative review"
            | "reviewed text reviewed"
            | "reviewed text review" => {
                return Some(WorkspaceActionId::DerivativeReviewed);
            }
            "derivative archive" | "reviewed text archive" => {
                return Some(WorkspaceActionId::DerivativeArchive);
            }
            "clip new" | "derivative clip" | "transcript clip" | "ocr clip" | "video clip"
            | "edi clip" | "billing clip" => return Some(WorkspaceActionId::ClipNew),
            "clip save" => return Some(WorkspaceActionId::ClipSave),
            "clip clear" => return Some(WorkspaceActionId::ClipClear),
            "clip select" | "clip include" => return Some(WorkspaceActionId::ClipSelect),
            "clip deselect" | "clip exclude" => return Some(WorkspaceActionId::ClipDeselect),
            "clip toggle" => return Some(WorkspaceActionId::ClipToggle),
            "clip inspect" => return Some(WorkspaceActionId::ClipInspect),
            "clip reviewed" | "clip review" => return Some(WorkspaceActionId::ClipReviewed),
            "clip archive" => return Some(WorkspaceActionId::ClipArchive),
            "workflow documents" | "workflow files" => {
                return Some(WorkspaceActionId::FocusDocuments);
            }
            "agent request"
            | "agent instructions"
            | "agent instruction"
            | "request agent"
            | "ask agent"
            | "ask codex about eval" => {
                return Some(WorkspaceActionId::AgentRequest);
            }
            "agent preview"
            | "medical agent plan"
            | "review medical agent plan"
            | "review what the agent will see"
            | "context packet"
            | "packet preview"
            | "preview codex packet" => {
                return Some(WorkspaceActionId::AgentPreview);
            }
            "agent inbox"
            | "agent results"
            | "results inbox"
            | "agent review"
            | "result review"
            | "returned work review"
            | "review codex result" => {
                return Some(WorkspaceActionId::AgentInbox);
            }
            "agent handoff inspect"
            | "agent context inspect"
            | "agent packet inspect"
            | "context packet inspect"
            | "packet inspect"
            | "packet replay"
            | "codex packet"
            | "packet compare"
            | "compare packet and result" => {
                return Some(WorkspaceActionId::AgentPacketInspect);
            }
            "agent result"
            | "agent result paste"
            | "result paste"
            | "codex result"
            | "returned work"
            | "returned work paste" => {
                return Some(WorkspaceActionId::AgentResult);
            }
            "agent result save" | "result save" | "returned work save" => {
                return Some(WorkspaceActionId::AgentResultSave);
            }
            "agent result clear" | "result clear" | "returned work clear" => {
                return Some(WorkspaceActionId::AgentResultClear);
            }
            "agent result inspect"
            | "agent result view"
            | "result inspect"
            | "codex result inspect"
            | "review result"
            | "packet result compare"
            | "compare codex packet and result"
            | "returned work inspect"
            | "returned work view" => {
                return Some(WorkspaceActionId::AgentResultInspect);
            }
            "agent result next" | "result next" | "returned work next" => {
                return Some(WorkspaceActionId::AgentResultNext);
            }
            "agent result reviewed"
            | "agent result review"
            | "result reviewed"
            | "returned work reviewed" => {
                return Some(WorkspaceActionId::AgentResultReviewed);
            }
            "agent result dismiss"
            | "result dismiss"
            | "result dismissed"
            | "returned work dismiss" => {
                return Some(WorkspaceActionId::AgentResultDismiss);
            }
            "agent result to proposal"
            | "result to proposal"
            | "agent result proposal"
            | "returned work to proposal"
            | "make proposal"
            | "proposal draft" => {
                return Some(WorkspaceActionId::AgentResultToProposal);
            }
            "agent result to addendum"
            | "result to addendum"
            | "agent result addendum"
            | "returned work to addendum"
            | "make addendum"
            | "addendum draft" => {
                return Some(WorkspaceActionId::AgentResultToAddendum);
            }
            "agent result to job"
            | "result to job"
            | "agent result job"
            | "returned work to job"
            | "make job"
            | "job draft" => {
                return Some(WorkspaceActionId::AgentResultToJob);
            }
            "agent clear" | "clear agent request" => return Some(WorkspaceActionId::AgentClear),
            "agent send"
            | "send agent request"
            | "switch to agent"
            | "submit medical agent plan" => {
                return Some(WorkspaceActionId::Handoff);
            }
            "jobs" | "focus jobs" | "workflow jobs" => return Some(WorkspaceActionId::FocusJobs),
            "timeline" | "workflow timeline" => return Some(WorkspaceActionId::FocusTimeline),
            "audit" | "provenance" | "workflow audit" => {
                return Some(WorkspaceActionId::FocusWorkflowAudit);
            }
            _ => {}
        }
    }
    let alias = match command.as_str() {
        "q" | "quit" | "exit" => "return",
        "send context" | "context" | "open codex handoff" => "handoff",
        "save chart" | "save chart workspace" => "save",
        "help" => "actions",
        "client create" => "client new",
        "patient create" => "patient new",
        "focus patients" => "patients",
        "patient details" | "details" | "focus patient details" | "demographics" => "demographics",
        "contact" | "contact edit" => "demographics edit",
        "coverage" => "coverage edit",
        "verify coverage" | "coverage card" | "insurance card" => "coverage verify",
        "title" | "note title" => "focus note title",
        "body" | "note body" | "write eval note" | "write first eval" => "focus note body",
        "workflow" | "practice workflow" | "clinical workspace" => "focus clinical workspace",
        "encounter create" | "visit open" | "open encounter" | "start encounter" => {
            "encounter open"
        }
        "sign note" => "note sign",
        "document add" | "attach document" | "document attach" | "file attach"
        | "artifact attach" => "file add",
        "file choose" | "choose file" | "drop file" | "drop patient file" => "file drop",
        "artifact import" | "document import" => "file import",
        "artifact thumbnail" | "document thumbnail" | "file preview" | "generate preview" => {
            "file thumbnail"
        }
        "open file" | "open local file" | "open patient file" => "file open",
        "artifact include" => "artifact select",
        "artifact exclude" => "artifact deselect",
        "derivative include" => "derivative select",
        "derivative exclude" => "derivative deselect",
        "clip include" => "clip select",
        "clip exclude" => "clip deselect",
        "proposal approve" => "proposal accept",
        "proposal reject" => "proposal decline",
        "task create" | "task new" => "job new",
        "job complete" | "task complete" | "task done" => "job done",
        "task cancel" => "job cancel",
        "task next" => "job next",
        _ => command.as_str(),
    };
    WORKSPACE_ACTIONS
        .iter()
        .find(|action| action.applies_to(profile) && action.command == alias)
        .map(|action| action.id)
}

pub(crate) fn normalize_workspace_command(input: &str) -> String {
    input
        .trim()
        .trim_start_matches(':')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}
