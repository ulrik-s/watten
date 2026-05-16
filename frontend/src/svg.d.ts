// Vite ships `?react` / raw / url loaders for SVG; for the plain
// `import url from './foo.svg'` form we just want TS to accept it as a
// string URL.
declare module '*.svg' {
  const src: string;
  export default src;
}
