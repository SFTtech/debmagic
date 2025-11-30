# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information
from datetime import date

project = "debmagic"
copyright = f"{date.today().year}, Debmagic Authors"
author = "Debmagic Authors"

master_doc = "index"
language = "en"

extensions = [
    "sphinx_copybutton",
    "myst_parser",
]
myst_enable_extensions = [
    "colon_fence",
]

templates_path = ["_templates"]
exclude_patterns = ["_build"]

# html settings

html_theme = "sphinx_rtd_theme"
html_static_path = ["_static"]
html_context = {
    "display_github": True,
    "github_user": "SFTtech",
    "github_repo": "debmagic",
    "github_version": "main",
    "conf_py_path": "/docs/",
}
