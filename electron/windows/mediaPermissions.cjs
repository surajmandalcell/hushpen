const { isAppUrl } = require("./appUrl.cjs");

function isTrustedOrigin(details, webContents, appUrl) {
  return [
    details.requestingUrl,
    details.requestingOrigin,
    details.securityOrigin,
    webContents?.getURL?.(),
  ].some((candidate) => isAppUrl(candidate, appUrl));
}

function isTrustedMediaPermission(webContents, permission, details = {}, appUrl) {
  if (permission !== "media") return false;

  const mediaType = details.mediaType || (Array.isArray(details.mediaTypes) ? details.mediaTypes[0] : undefined);
  if (mediaType && mediaType !== "audio" && mediaType !== "unknown") {
    return false;
  }

  return isTrustedOrigin(details, webContents, appUrl);
}

// `navigator.clipboard.writeText` needs this permission once a permission handler is
// installed. Reading the clipboard stays denied: nothing in the app needs it.
function isTrustedClipboardWrite(webContents, permission, details = {}, appUrl) {
  return permission === "clipboard-sanitized-write" && isTrustedOrigin(details, webContents, appUrl);
}

function isAllowedPermission(webContents, permission, details, appUrl) {
  return isTrustedMediaPermission(webContents, permission, details, appUrl)
    || isTrustedClipboardWrite(webContents, permission, details, appUrl);
}

function configureMediaPermissions({ appUrl }) {
  const { session } = require("electron");

  session.defaultSession.setPermissionRequestHandler((webContents, permission, callback, details = {}) => {
    callback(isAllowedPermission(webContents, permission, details, appUrl));
  });

  session.defaultSession.setPermissionCheckHandler((webContents, permission, requestingOrigin, details = {}) => (
    isAllowedPermission(webContents, permission, {
      ...details,
      requestingOrigin,
    }, appUrl)
  ));
}

module.exports = { configureMediaPermissions, isAllowedPermission, isTrustedClipboardWrite, isTrustedMediaPermission };
