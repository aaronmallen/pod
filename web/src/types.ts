export type NoteTone = 'plasma' | 'success' | 'warning' | 'muted';

export interface Note {
  tag: string;
  tone: NoteTone;
  text: string;
}

export interface Feature {
  id: string;
  icon: string;
  title: string;
  line: string;
  soon?: boolean;
}

export type BuildId =
  | 'mac-arm'
  | 'mac-x64'
  | 'win-x64-exe'
  | 'win-x64-msi'
  | 'lin-deb'
  | 'lin-app'
  | 'lin-flatpak';

export interface BuildAsset {
  id: BuildId;
  arch: string;
  ext: string;
  size: string;
  downloadUrl: string;
  filename: string;
}

export interface Platform {
  id: string;
  name: string;
  sub: string;
  builds: BuildAsset[];
}

export type PlatformBuilds = Platform;

export interface ReleaseData {
  version: string;
  channel: 'stable' | 'beta' | 'nightly';
  build: string;
  date: string;
  notesAnchor: string;
}

export type Release = Pick<ReleaseData, 'version' | 'date' | 'channel'>;
