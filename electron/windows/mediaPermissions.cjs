function isTrustedAppUrl(value = "", devServerUrl) {
  if (!value) return false;

  try {
    const url = new URL(value);
    const devUrl = new URL(devServerUrl);
    return url.protocol === "file:" || url.origin === devUrl.origin;
  } catch {
    return value.startsWith("file://") || value.startsWith(devServerUrl);
  }
}

function isTrustedMediaPermission(webContents, permission, details = {}, devServerUrl) {
  if (permission !== "media") return false;

  const mediaType = details.mediaType || (Array.isArray(details.mediaTypes) ? details.mediaTypes[0] : undefined);
  if (mediaType && mediaType !== "audio" && mediaType !== "unknown") {
    return false;
  }

  return [
    details.requestingUrl,
    details.requestingOrigin,
    details.securityOrigin,
    webContents?.getURL?.(),
  ].some((candidate) => isTrustedAppUrl(candidate, devServerUrl));
}

function configureMediaPermissions({ devServerUrl }) {
  const { session } = require("electron");

  session.defaultSession.setPermissionRequestHandler((webContents, permission, callback, details = {}) => {
    callback(isTrustedMediaPermission(webContents, permission, details, devServerUrl));
  });

  session.defaultSession.setPermissionCheckHandler((webContents, permission, requestingOrigin, details = {}) => (
    isTrustedMediaPermission(webContents, permission, {
      ...details,
      requestingOrigin,
    }, devServerUrl)
  ));
}

module.exports = { configureMediaPermissions, isTrustedAppUrl, isTrustedMediaPermission };
