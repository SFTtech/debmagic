//! Debconf template and maintainer-script Tags from lintian's `debian/debconf` Check.
//!
//! `templates` parses Control `templates` as debconf Deb822. `scripts` walks
//! `config`, `prerm`, `postrm`, `preinst`, and `postinst` with that Check's
//! line counter. Each Rule owns one Tag.

mod files;
mod ln0236_unknown_field_in_templates;
mod ln0237_possible_debconf_note_abuse;
mod ln0238_malformed_question_in_templates;
mod ln0239_unknown_debconf_priority;
mod ln0240_using_imperative_form_in_templates;
mod ln0241_loads_obsolete_confmodule;
mod ln0242_making_assumptions_about_interfaces_in_templates;
mod ln0243_malformed_prompt_in_templates;
mod ln0244_malformed_title_in_templates;
mod ln0245_mismatch_translated_choices;
mod ln0246_missing_debconf_dependency_for_preinst;
mod ln0247_no_debconf_templates;
mod satisfy;
mod scripts;
mod templates;

pub(crate) use ln0236_unknown_field_in_templates::UnknownFieldInTemplates;
pub(crate) use ln0237_possible_debconf_note_abuse::PossibleDebconfNoteAbuse;
pub(crate) use ln0238_malformed_question_in_templates::MalformedQuestionInTemplates;
pub(crate) use ln0239_unknown_debconf_priority::UnknownDebconfPriority;
pub(crate) use ln0240_using_imperative_form_in_templates::UsingImperativeFormInTemplates;
pub(crate) use ln0241_loads_obsolete_confmodule::LoadsObsoleteConfmodule;
pub(crate) use ln0242_making_assumptions_about_interfaces_in_templates::MakingAssumptionsAboutInterfacesInTemplates;
pub(crate) use ln0243_malformed_prompt_in_templates::MalformedPromptInTemplates;
pub(crate) use ln0244_malformed_title_in_templates::MalformedTitleInTemplates;
pub(crate) use ln0245_mismatch_translated_choices::MismatchTranslatedChoices;
pub(crate) use ln0246_missing_debconf_dependency_for_preinst::MissingDebconfDependencyForPreinst;
pub(crate) use ln0247_no_debconf_templates::NoDebconfTemplates;
