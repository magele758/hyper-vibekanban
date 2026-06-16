import { NodeKey } from 'lexical';
import {
  createDecoratorNode,
  type DecoratorNodeConfig,
  type GeneratedDecoratorNode,
} from '@vibe/ui/components/create-decorator-node';
import { useTheme, getResolvedTheme } from '@/shared/hooks/useTheme';
import { MermaidDiagram } from './MermaidDiagram';

/** Data model for a mermaid diagram, serialized as a ```mermaid fenced block. */
export interface MermaidData {
  chart: string;
}

function MermaidNodeComponent({
  data,
}: {
  data: MermaidData;
  nodeKey: NodeKey;
}) {
  const { theme } = useTheme();
  return <MermaidDiagram chart={data.chart} theme={getResolvedTheme(theme)} />;
}

const config: DecoratorNodeConfig<MermaidData> = {
  type: 'mermaid',
  serialization: {
    format: 'fenced',
    language: 'mermaid',
    serialize: (data) => data.chart,
    deserialize: (content) => ({ chart: content }),
    validate: (data) => data.chart.trim().length > 0,
  },
  component: MermaidNodeComponent,
  keyboardSelectable: false,
};

const result = createDecoratorNode(config);

export const MermaidNode = result.Node;
export type MermaidNodeInstance = GeneratedDecoratorNode<MermaidData>;
export const $createMermaidNode = result.createNode;
export const $isMermaidNode = result.isNode;
export const [MERMAID_EXPORT_TRANSFORMER, MERMAID_TRANSFORMER] =
  result.transformers;
