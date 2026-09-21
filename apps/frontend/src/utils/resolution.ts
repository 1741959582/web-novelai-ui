export type ResolutionFamily = "small" | "normal" | "large" | "wallpaper";
export type Orientation = "landscape" | "portrait" | "square";

export const RESOLUTION_TABLE: Record<ResolutionFamily, Record<Orientation, [number, number]>> = {
  small: {
    landscape: [768, 512],
    portrait: [512, 768],
    square: [640, 640],
  },
  normal: {
    landscape: [1216, 832],
    portrait: [832, 1216],
    square: [1024, 1024],
  },
  large: {
    landscape: [1536, 1024],
    portrait: [1024, 1536],
    square: [1472, 1472],
  },
  wallpaper: {
    landscape: [1920, 1088],
    portrait: [1088, 1920],
    square: [1472, 1472],
  },
};

export const RESOLUTION_LABELS: { id: ResolutionFamily; label: string }[] = [
  { id: "small", label: "Small" },
  { id: "normal", label: "Normal" },
  { id: "large", label: "Large" },
  { id: "wallpaper", label: "Wallpaper" },
];

export function inferResolution(width: number, height: number): { family: ResolutionFamily; orientation: Orientation } {
  for (const family of Object.keys(RESOLUTION_TABLE) as ResolutionFamily[]) {
    for (const orientation of ["landscape", "portrait", "square"] as Orientation[]) {
      const [w, h] = RESOLUTION_TABLE[family][orientation];
      if (w === width && h === height) return { family, orientation };
    }
  }
  if (width === height) return { family: "normal", orientation: "square" };
  return { family: "normal", orientation: width > height ? "landscape" : "portrait" };
}

export function applyResolution(family: ResolutionFamily, orientation: Orientation) {
  const [width, height] = RESOLUTION_TABLE[family][orientation];
  return { width, height };
}
