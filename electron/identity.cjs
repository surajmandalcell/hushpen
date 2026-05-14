const APP_ID = "com.surajmandal.asrpro";
const APP_NAME = "ASR Pro";
const APP_COPYRIGHT = "(c) 2026 Suraj Mandal";

function buildAboutPanelOptions(version) {
  return {
    applicationName: APP_NAME,
    applicationVersion: version,
    version,
    copyright: APP_COPYRIGHT,
    authors: ["Suraj Mandal"],
  };
}

module.exports = {
  APP_COPYRIGHT,
  APP_ID,
  APP_NAME,
  buildAboutPanelOptions,
};
