export async function resolve(specifier, context, defaultResolve) {
  try {
    return await defaultResolve(specifier, context, defaultResolve);
  } catch (error) {
    if (!specifier.startsWith(".") && !specifier.startsWith("/")) {
      throw error;
    }

    for (const candidate of [`${specifier}.ts`, `${specifier}/index.ts`]) {
      try {
        return await defaultResolve(candidate, context, defaultResolve);
      } catch {}
    }

    throw error;
  }
}
