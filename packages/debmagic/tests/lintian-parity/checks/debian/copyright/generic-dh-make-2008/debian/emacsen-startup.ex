;; -*-emacs-lisp-*-
;;
;; Emacs startup file, e.g.  /etc/emacs/site-start.d/50generic-dh-make-2008.el
;; for the Debian generic-dh-make-2008 package
;;
;; Originally contributed by Nils Naumann <naumann@unileoben.ac.at>
;; Modified by Dirk Eddelbuettel <edd@debian.org>
;; Adapted for dh-make by Jim Van Zandt <jrv@debian.org>

;; The generic-dh-make-2008 package follows the Debian/GNU Linux 'emacsen' policy and
;; byte-compiles its elisp files for each 'emacs flavor' (emacs19,
;; xemacs19, emacs20, xemacs20...).  The compiled code is then
;; installed in a subdirectory of the respective site-lisp directory.
;; We have to add this to the load-path:
(let ((package-dir (concat "/usr/share/"
                           (symbol-name flavor)
                           "/site-lisp/generic-dh-make-2008")))
;; If package-dir does not exist, the generic-dh-make-2008 package must have
;; removed but not purged, and we should skip the setup.
  (when (file-directory-p package-dir)
        (setq load-path (cons package-dir load-path))
       (autoload 'generic-dh-make-2008-mode "generic-dh-make-2008-mode"
         "Major mode for editing generic-dh-make-2008 files." t)
       (add-to-list 'auto-mode-alist '("\\.generic-dh-make-2008$" . generic-dh-make-2008-mode))))

