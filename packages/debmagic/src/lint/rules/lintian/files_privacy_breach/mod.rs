//! Rules for lintian's `files/privacy-breach` Check.
//!
//! One scan of a package's HTML, CSS, JavaScript and XML files (see `engine`)
//! serves every Rule; each Rule reports the hits of its own Tag.

mod engine;
mod ln0640_privacy_breach_generic;
mod ln0641_privacy_breach_uses_embedded_file;
mod ln0642_privacy_breach_google_adsense;
mod ln0643_privacy_breach_donation;
mod ln0644_privacy_breach_google_cse;
mod ln0645_privacy_breach_facebook;
mod ln0646_privacy_breach_google_plus;
mod ln0647_privacy_breach_logo;
mod ln0648_privacy_breach_statistics_website;
mod ln0649_privacy_breach_piwik;
mod ln0650_privacy_breach_twitter;

pub(crate) use ln0640_privacy_breach_generic::PrivacyBreachGeneric;
pub(crate) use ln0641_privacy_breach_uses_embedded_file::PrivacyBreachUsesEmbeddedFile;
pub(crate) use ln0642_privacy_breach_google_adsense::PrivacyBreachGoogleAdsense;
pub(crate) use ln0643_privacy_breach_donation::PrivacyBreachDonation;
pub(crate) use ln0644_privacy_breach_google_cse::PrivacyBreachGoogleCse;
pub(crate) use ln0645_privacy_breach_facebook::PrivacyBreachFacebook;
pub(crate) use ln0646_privacy_breach_google_plus::PrivacyBreachGooglePlus;
pub(crate) use ln0647_privacy_breach_logo::PrivacyBreachLogo;
pub(crate) use ln0648_privacy_breach_statistics_website::PrivacyBreachStatisticsWebsite;
pub(crate) use ln0649_privacy_breach_piwik::PrivacyBreachPiwik;
pub(crate) use ln0650_privacy_breach_twitter::PrivacyBreachTwitter;
