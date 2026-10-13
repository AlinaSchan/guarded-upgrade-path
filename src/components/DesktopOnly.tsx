import { LogosMark, LogosWordmark } from "./LogosBrand";

export const RELEASES_URL = "https://github.com/AlinaSchan/guarded-upgrade-path/releases/latest";

/** Rendered when the interface is opened outside the desktop shell. There is no web version. */
export function DesktopOnly() {
  return (
    <div className="desktop-only">
      <div className="brand-logo">
        <LogosMark size={26} />
        <LogosWordmark height={15} />
      </div>
      <h1>Guarded Upgrade Path is a desktop application</h1>
      <p>
        This interface runs only inside the Windows or macOS build, where the Rust engine keeps the workspace and makes every authority decision. It is not a
        web application, and there is no Linux version.
      </p>
      <p>
        Installers: <a href={RELEASES_URL}>{RELEASES_URL}</a>
      </p>
    </div>
  );
}
