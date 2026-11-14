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
mod ln0316_no_template_description;
mod ln0317_no_template_type;
mod ln0318_select_with_boolean_choices;
mod ln0319_template_uses_unsplit_choices;
mod ln0320_too_long_extended_description_in_templates;
mod ln0321_too_long_short_description_in_templates;
mod ln0322_using_first_person_in_templates;
mod ln0323_using_question_in_extended_description_in_templates;
mod ln0324_postinst_does_not_load_confmodule;
mod ln0325_postinst_uses_db_input;
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
pub(crate) use ln0316_no_template_description::NoTemplateDescription;
pub(crate) use ln0317_no_template_type::NoTemplateType;
pub(crate) use ln0318_select_with_boolean_choices::SelectWithBooleanChoices;
pub(crate) use ln0319_template_uses_unsplit_choices::TemplateUsesUnsplitChoices;
pub(crate) use ln0320_too_long_extended_description_in_templates::TooLongExtendedDescriptionInTemplates;
pub(crate) use ln0321_too_long_short_description_in_templates::TooLongShortDescriptionInTemplates;
pub(crate) use ln0322_using_first_person_in_templates::UsingFirstPersonInTemplates;
pub(crate) use ln0323_using_question_in_extended_description_in_templates::UsingQuestionInExtendedDescriptionInTemplates;
pub(crate) use ln0324_postinst_does_not_load_confmodule::PostinstDoesNotLoadConfmodule;
pub(crate) use ln0325_postinst_uses_db_input::PostinstUsesDbInput;
