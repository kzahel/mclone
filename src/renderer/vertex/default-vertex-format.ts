import { VertexFormatElement } from "./vertex-format-element";
import { VertexFormat } from "./vertex-format";

function immutableMap(entries: ReadonlyArray<readonly [string, VertexFormatElement]>): ReadonlyMap<string, VertexFormatElement> {
  return new Map(entries);
}

export class DefaultVertexFormat {
  public static readonly ELEMENT_POSITION = new VertexFormatElement(
    0,
    VertexFormatElement.Type.FLOAT,
    VertexFormatElement.Usage.POSITION,
    3,
  );

  public static readonly ELEMENT_COLOR = new VertexFormatElement(
    0,
    VertexFormatElement.Type.UBYTE,
    VertexFormatElement.Usage.COLOR,
    4,
  );

  public static readonly ELEMENT_UV0 = new VertexFormatElement(
    0,
    VertexFormatElement.Type.FLOAT,
    VertexFormatElement.Usage.UV,
    2,
  );

  public static readonly ELEMENT_UV1 = new VertexFormatElement(
    1,
    VertexFormatElement.Type.SHORT,
    VertexFormatElement.Usage.UV,
    2,
  );

  public static readonly ELEMENT_UV2 = new VertexFormatElement(
    2,
    VertexFormatElement.Type.SHORT,
    VertexFormatElement.Usage.UV,
    2,
  );

  public static readonly ELEMENT_NORMAL = new VertexFormatElement(
    0,
    VertexFormatElement.Type.BYTE,
    VertexFormatElement.Usage.NORMAL,
    3,
  );

  public static readonly ELEMENT_PADDING = new VertexFormatElement(
    0,
    VertexFormatElement.Type.BYTE,
    VertexFormatElement.Usage.PADDING,
    1,
  );

  public static readonly ELEMENT_UV = DefaultVertexFormat.ELEMENT_UV0;

  public static readonly BLIT_SCREEN = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["UV", DefaultVertexFormat.ELEMENT_UV],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
    ]),
  );

  public static readonly BLOCK = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["UV2", DefaultVertexFormat.ELEMENT_UV2],
      ["Normal", DefaultVertexFormat.ELEMENT_NORMAL],
      ["Padding", DefaultVertexFormat.ELEMENT_PADDING],
    ]),
  );

  public static readonly NEW_ENTITY = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["UV1", DefaultVertexFormat.ELEMENT_UV1],
      ["UV2", DefaultVertexFormat.ELEMENT_UV2],
      ["Normal", DefaultVertexFormat.ELEMENT_NORMAL],
      ["Padding", DefaultVertexFormat.ELEMENT_PADDING],
    ]),
  );

  public static readonly PARTICLE = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["UV2", DefaultVertexFormat.ELEMENT_UV2],
    ]),
  );

  public static readonly POSITION = new VertexFormat(immutableMap([["Position", DefaultVertexFormat.ELEMENT_POSITION]]));

  public static readonly POSITION_COLOR = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
    ]),
  );

  public static readonly POSITION_COLOR_NORMAL = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["Normal", DefaultVertexFormat.ELEMENT_NORMAL],
      ["Padding", DefaultVertexFormat.ELEMENT_PADDING],
    ]),
  );

  public static readonly POSITION_COLOR_LIGHTMAP = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["UV2", DefaultVertexFormat.ELEMENT_UV2],
    ]),
  );

  public static readonly POSITION_TEX = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
    ]),
  );

  public static readonly POSITION_COLOR_TEX = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
    ]),
  );

  public static readonly POSITION_TEX_COLOR = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
    ]),
  );

  public static readonly POSITION_COLOR_TEX_LIGHTMAP = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["UV2", DefaultVertexFormat.ELEMENT_UV2],
    ]),
  );

  public static readonly POSITION_TEX_LIGHTMAP_COLOR = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["UV2", DefaultVertexFormat.ELEMENT_UV2],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
    ]),
  );

  public static readonly POSITION_TEX_COLOR_NORMAL = new VertexFormat(
    immutableMap([
      ["Position", DefaultVertexFormat.ELEMENT_POSITION],
      ["UV0", DefaultVertexFormat.ELEMENT_UV0],
      ["Color", DefaultVertexFormat.ELEMENT_COLOR],
      ["Normal", DefaultVertexFormat.ELEMENT_NORMAL],
      ["Padding", DefaultVertexFormat.ELEMENT_PADDING],
    ]),
  );
}
