import JSON5 from "json5";
import { OPENAPI_DYNAMIC_VARIABLES } from "./openapiDynamicVariables";

const primaryGroups = ["string", "lorem", "number", "date", "person", "phone", "internet", "image", "location", "helpers", "common"];
export const dynamicGeneratorCatalog = [...new Set([...primaryGroups, ...OPENAPI_DYNAMIC_VARIABLES.map(({ group }) => group)])].map((group) => ({ group, methods: OPENAPI_DYNAMIC_VARIABLES.filter((item) => item.group === group) })).filter(({ methods }) => methods.length > 0);

export type GeneratorField = { name: string; kind: "number" | "text" | "literal" | "boolean"; choices?: readonly string[] };

const parameters: Record<string, string> = {
  "number.int": "min:number max:number multipleOf:number",
  "number.float": "min:number max:number multipleOf:number fractionDigits:number",
  "number.bigInt": "min max multipleOf",
  "number.binary": "min:number max:number",
  "number.octal": "min:number max:number",
  "number.hex": "min:number max:number",
  "string.alpha": "length:literal casing:upper,lower,mixed exclude",
  "string.alphanumeric": "length:literal casing:upper,lower,mixed exclude",
  "string.numeric": "length:literal allowLeadingZeros:boolean exclude",
  "string.binary": "length:literal prefix",
  "string.octal": "length:literal prefix",
  "string.hexadecimal": "length:literal casing:upper,lower,mixed prefix",
  "string.nanoid": "length:literal",
  "string.sample": "length:literal",
  "string.symbol": "length:literal",
  "string.fromCharacters": "characters length:literal",
  "datatype.boolean": "probability:number",
  "date.between": "from to",
  "date.betweens": "from to count:literal",
  "date.birthdate": "mode:age,year min:number max:number refDate",
  "date.future": "years:number refDate",
  "date.past": "years:number refDate",
  "date.recent": "days:number refDate",
  "date.soon": "days:number refDate",
  "date.month": "abbreviated:boolean context:boolean",
  "date.weekday": "abbreviated:boolean context:boolean",
  "person.fullName": "firstName lastName sex:female,male",
  "person.firstName": "sex:female,male",
  "person.lastName": "sex:female,male",
  "person.middleName": "sex:female,male",
  "internet.email": "firstName lastName provider allowSpecialCharacters:boolean",
  "internet.password": "length:number memorable:boolean prefix",
  "internet.url": "appendSlash:boolean protocol:http,https",
  "location.zipCode": "format state",
  "location.streetAddress": "useFullAddress:boolean",
  "image.url": "width:number height:number",
  "image.dataUri": "width:number height:number color",
  "commerce.price": "min:number max:number dec:number symbol",
  "helpers.fromRegExp": "pattern",
  "helpers.arrayElement": "array:literal",
  "helpers.arrayElements": "array:literal count:literal",
  "helpers.rangeToNumber": "min:number max:number",
  "helpers.replaceSymbols": "string",
  "helpers.replaceCreditCardSymbols": "string symbol",
  "helpers.slugify": "string",
  "helper.autoIncrementEnvVar": "name start:number step:number",
  randomInt: "min:number max:number",
};

const countMethods = new Set(["lorem.words", "lorem.lines", "lorem.sentences", "lorem.paragraphs", "word.words"]);
export function dynamicGeneratorFields(method: string): GeneratorField[] {
  const definition = parameters[method] ?? (countMethods.has(method) ? "count:literal min:number max:number" : "");
  return definition
    .split(" ")
    .filter(Boolean)
    .map((entry) => {
      const [name, type] = entry.split(":");
      if (type === "number" || type === "boolean" || type === "literal") return { name: name!, kind: type };
      return { name: name!, kind: "text", choices: type?.split(",") };
    });
}

type Processor = { name: string; input?: "number" | "text"; defaultValue?: string };
export const dynamicGeneratorProcessors: Processor[] = [
  { name: "upperCase" },
  { name: "lowerCase" },
  { name: "trim" },
  { name: "urlencode" },
  { name: "base64" },
  { name: "md5" },
  { name: "sha1" },
  { name: "sha256" },
  { name: "sha512" },
  { name: "json" },
  { name: "format", input: "text", defaultValue: "yyyy-MM-dd HH:mm:ss" },
  { name: "formatISO" },
  { name: "formatISO9075" },
  { name: "formatRFC3339" },
  { name: "formatRFC7231" },
  ...["addYears", "addQuarters", "addMonths", "addWeeks", "addDays", "addHours", "addMinutes", "addSeconds"].map((name): Processor => ({ name, input: "number", defaultValue: "1" })),
];

export type GeneratorProcessor = { name: string; value: string };

export function buildDynamicGeneratorExpression(method: string, values: Record<string, string>, processors: GeneratorProcessor[]): string {
  if (!method) return "";
  const args = dynamicGeneratorFields(method).flatMap(({ name, kind }) => {
    const raw = values[name];
    if (raw === undefined || !raw.trim()) return [];
    const value: unknown = kind === "text" ? raw : JSON5.parse(raw);
    if ((kind === "number" && (typeof value !== "number" || !Number.isFinite(value))) || (kind === "boolean" && typeof value !== "boolean")) throw new TypeError(name);
    return [`${name}=${JSON.stringify(value)}`];
  });
  const pipeline = processors
    .map(({ name, value }) => {
      const processor = dynamicGeneratorProcessors.find((option) => option.name === name);
      if (!processor) throw new TypeError(name);
      const argument = processor.input === "number" ? Number(value) : value;
      if (processor.input === "number" && (!value.trim() || !Number.isFinite(argument))) throw new TypeError(name);
      return `|${name}(${processor.input ? JSON.stringify(argument) : ""})`;
    })
    .join("");
  return `{{$${method}${args.length ? `(${args.join(",")})` : ""}${pipeline}}}`;
}
