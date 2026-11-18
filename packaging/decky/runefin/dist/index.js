const manifest = {"name":"Runefin"};
const API_VERSION = 2;
const internalAPIConnection = window.__DECKY_SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED_deckyLoaderAPIInit;
if (!internalAPIConnection) {
    throw new Error('[@decky/api]: Failed to connect to the loader as as the loader API was not initialized. This is likely a bug in Decky Loader.');
}
let api;
try {
    api = internalAPIConnection.connect(API_VERSION, manifest.name);
}
catch {
    api = internalAPIConnection.connect(1, manifest.name);
    console.warn(`[@decky/api] Requested API version ${API_VERSION} but the running loader only supports version 1. Some features may not work.`);
}
if (api._version != API_VERSION) {
    console.warn(`[@decky/api] Requested API version ${API_VERSION} but the running loader only supports version ${api._version}. Some features may not work.`);
}
const callable = api.callable;
const toaster = api.toaster;
const definePlugin = (fn) => {
    return (...args) => {
        return fn(...args);
    };
};

var DefaultContext = {
  color: undefined,
  size: undefined,
  className: undefined,
  style: undefined,
  attr: undefined
};
var IconContext = SP_REACT.createContext && /*#__PURE__*/SP_REACT.createContext(DefaultContext);

var _excluded = ["attr", "size", "title"];
function _objectWithoutProperties(e, t) { if (null == e) return {}; var o, r, i = _objectWithoutPropertiesLoose(e, t); if (Object.getOwnPropertySymbols) { var n = Object.getOwnPropertySymbols(e); for (r = 0; r < n.length; r++) o = n[r], -1 === t.indexOf(o) && {}.propertyIsEnumerable.call(e, o) && (i[o] = e[o]); } return i; }
function _objectWithoutPropertiesLoose(r, e) { if (null == r) return {}; var t = {}; for (var n in r) if ({}.hasOwnProperty.call(r, n)) { if (-1 !== e.indexOf(n)) continue; t[n] = r[n]; } return t; }
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
function ownKeys(e, r) { var t = Object.keys(e); if (Object.getOwnPropertySymbols) { var o = Object.getOwnPropertySymbols(e); r && (o = o.filter(function (r) { return Object.getOwnPropertyDescriptor(e, r).enumerable; })), t.push.apply(t, o); } return t; }
function _objectSpread(e) { for (var r = 1; r < arguments.length; r++) { var t = null != arguments[r] ? arguments[r] : {}; r % 2 ? ownKeys(Object(t), true).forEach(function (r) { _defineProperty(e, r, t[r]); }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(t)) : ownKeys(Object(t)).forEach(function (r) { Object.defineProperty(e, r, Object.getOwnPropertyDescriptor(t, r)); }); } return e; }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: true, configurable: true, writable: true }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == typeof i ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != typeof t || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r); if ("object" != typeof i) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
function Tree2Element(tree) {
  return tree && tree.map((node, i) => /*#__PURE__*/SP_REACT.createElement(node.tag, _objectSpread({
    key: i
  }, node.attr), Tree2Element(node.child)));
}
function GenIcon(data) {
  return props => /*#__PURE__*/SP_REACT.createElement(IconBase, _extends({
    attr: _objectSpread({}, data.attr)
  }, props), Tree2Element(data.child));
}
function IconBase(props) {
  var elem = conf => {
    var attr = props.attr,
      size = props.size,
      title = props.title,
      svgProps = _objectWithoutProperties(props, _excluded);
    var computedSize = size || conf.size || "1em";
    var className;
    if (conf.className) className = conf.className;
    if (props.className) className = (className ? className + " " : "") + props.className;
    return /*#__PURE__*/SP_REACT.createElement("svg", _extends({
      stroke: "currentColor",
      fill: "currentColor",
      strokeWidth: "0"
    }, conf.attr, attr, svgProps, {
      className: className,
      style: _objectSpread(_objectSpread({
        color: props.color || conf.color
      }, conf.style), props.style),
      height: computedSize,
      width: computedSize,
      xmlns: "http://www.w3.org/2000/svg"
    }), title && /*#__PURE__*/SP_REACT.createElement("title", null, title), props.children);
  };
  return IconContext !== undefined ? /*#__PURE__*/SP_REACT.createElement(IconContext.Consumer, null, conf => elem(conf)) : elem(DefaultContext);
}

// THIS FILE IS AUTO GENERATED
function FaTv (props) {
  return GenIcon({"attr":{"viewBox":"0 0 640 512"},"child":[{"tag":"path","attr":{"d":"M592 0H48A48 48 0 0 0 0 48v320a48 48 0 0 0 48 48h240v32H112a16 16 0 0 0-16 16v32a16 16 0 0 0 16 16h416a16 16 0 0 0 16-16v-32a16 16 0 0 0-16-16H352v-32h240a48 48 0 0 0 48-48V48a48 48 0 0 0-48-48zm-16 352H64V64h512z"},"child":[]}]})(props);
}

const getStatus = callable("status");
const getInstallation = callable("installation");
const sendCommand = callable("command");
const APP_NAME = "Runefin";
function findRunefin() {
    return appStore.allApps.find((app) => app.BIsShortcut() && app.display_name === APP_NAME);
}
function wait(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
/** Launch the host installer's stable tile, or create one from its wrapper. */
async function openRunefin(wrapper, running) {
    if (running) {
        await sendCommand("raise");
        return;
    }
    const existing = findRunefin();
    if (existing) {
        SteamClient.Apps.RunGame(existing.gameid, "", -1, 100);
        return;
    }
    if (!wrapper) {
        throw new Error("Install the Steam tile first (scripts/steam-install.py).");
    }
    const appid = await SteamClient.Apps.AddShortcut(APP_NAME, wrapper, "", "");
    for (let attempt = 0; attempt < 20; attempt++) {
        await wait(100);
        const created = appStore.GetAppOverviewByAppID(appid);
        if (!created)
            continue;
        // AddShortcut can ignore some fields on the first call; set them once the overview exists.
        SteamClient.Apps.SetShortcutName(appid, APP_NAME);
        SteamClient.Apps.SetShortcutExe(appid, wrapper);
        SteamClient.Apps.SetShortcutLaunchOptions(appid, "");
        SteamClient.Apps.SetShortcutStartDir(appid, "");
        await wait(100);
        const ready = appStore.GetAppOverviewByAppID(appid);
        if (ready) {
            SteamClient.Apps.RunGame(ready.gameid, "", -1, 100);
            return;
        }
    }
    throw new Error("Steam did not create the Runefin shortcut.");
}
function showError(error) {
    toaster.toast({
        title: "Runefin",
        body: error instanceof Error ? error.message : String(error),
    });
}
function Content() {
    const [status, setStatus] = SP_REACT.useState({
        running: false,
        playback: "",
    });
    const [installation, setInstallation] = SP_REACT.useState({
        installed: false,
        wrapper: "",
        flatpak: false,
    });
    const [busy, setBusy] = SP_REACT.useState(false);
    const refresh = async () => {
        try {
            const next = await getStatus();
            setStatus(next);
        }
        catch {
            setStatus({ running: false, playback: "" });
        }
    };
    SP_REACT.useEffect(() => {
        getInstallation().then(setInstallation).catch(showError);
        refresh();
        const timer = setInterval(refresh, 2000);
        return () => clearInterval(timer);
    }, []);
    const run = async (action) => {
        setBusy(true);
        try {
            await action();
            await refresh();
        }
        catch (error) {
            showError(error);
        }
        finally {
            setBusy(false);
        }
    };
    const transport = (action) => run(async () => {
        await sendCommand(action);
    });
    const open = () => run(async () => {
        await openRunefin(installation.wrapper, status.running);
    });
    const addToMedia = () => run(async () => {
        const app = findRunefin();
        if (!app)
            throw new Error("Open Runefin once first.");
        const store = collectionStore;
        const existing = store.userCollections?.find((collection) => collection.displayName.toLowerCase() === "media");
        const collection = existing ?? store.NewUnsavedCollection("Media", undefined, []);
        const editable = collection.AsDragDropCollection();
        if (!editable)
            throw new Error("Steam did not expose the Media collection for editing.");
        editable.AddApps([app]);
        await collection.Save();
        toaster.toast({ title: "Runefin", body: "Added to the Media collection." });
    });
    const state = !status.running
        ? "Not running"
        : status.playback === "Playing"
            ? "Playing"
            : status.playback === "Paused"
                ? "Paused"
                : "Open";
    return (SP_JSX.jsxs(DFL.PanelSection, { children: [SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsxs(DFL.Focusable, { style: { display: "flex", alignItems: "center", gap: "8px" }, children: [SP_JSX.jsx("span", { style: { fontSize: "1.2rem", fontWeight: 600 }, children: APP_NAME }), SP_JSX.jsx("span", { style: { opacity: 0.7, marginLeft: "auto" }, children: state })] }) }), SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsx(DFL.DialogButton, { onClick: open, disabled: busy, children: status.running ? "Show Runefin" : "Open Runefin" }) }), status.running && (SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsxs(DFL.Focusable, { style: { display: "flex", gap: "8px" }, children: [SP_JSX.jsx(DFL.DialogButton, { style: { flex: 1 }, disabled: busy, onClick: () => transport("previous"), children: "\u25C0\u25C0" }), SP_JSX.jsx(DFL.DialogButton, { style: { flex: 1 }, disabled: busy, onClick: () => transport("playpause"), children: status.playback === "Playing" ? "Pause" : "Play" }), SP_JSX.jsx(DFL.DialogButton, { style: { flex: 1 }, disabled: busy, onClick: () => transport("next"), children: "\u25B6\u25B6" })] }) })), status.running && (SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsxs(DFL.Focusable, { style: { display: "flex", gap: "8px" }, children: [SP_JSX.jsx(DFL.DialogButton, { style: { flex: 1 }, disabled: busy, onClick: () => transport("stop"), children: "Stop" }), SP_JSX.jsx(DFL.DialogButton, { style: { flex: 1 }, disabled: busy, onClick: () => transport("quit"), children: "Quit to Steam" })] }) })), SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsx(DFL.DialogButton, { onClick: addToMedia, disabled: busy, children: "Add to Media collection" }) }), !installation.installed && (SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsx("span", { style: { opacity: 0.7 }, children: "The Steam tile is not installed. Run scripts/steam-install.py on the host once." }) })), !status.running && (SP_JSX.jsx(DFL.PanelSectionRow, { children: SP_JSX.jsx("span", { style: { opacity: 0.6 }, children: "Transport controls appear while Runefin is open. HDR remains a Desktop Mode feature; this panel does not change the compositor." }) }))] }));
}
var index = definePlugin(() => ({
    name: APP_NAME,
    content: SP_JSX.jsx(Content, {}),
    icon: SP_JSX.jsx(FaTv, {}),
}));

export { index as default };
//# sourceMappingURL=index.js.map
