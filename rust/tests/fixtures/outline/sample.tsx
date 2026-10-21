import React from "react";

type Props = { title: string };

/** Renders a title. */
export function Title({ title }: Props) {
  return <h1>{title}</h1>;
}

export const Card = ({ title }: Props): JSX.Element => <div>{title}</div>;

export class Panel extends React.Component<Props> {
  render() {
    return <section />;
  }
}
