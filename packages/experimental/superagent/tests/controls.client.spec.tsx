// @vitest-environment jsdom
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { expect, it, vi } from 'vitest'
import { DisclosureRow, InfoTip, Modal, SegmentedControl, SegmentedTabs } from '../src/client/controls.tsx'

declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean | undefined
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true

it('imports no DSH client runtime package, only React and type declarations', () => {
  const dir = join(import.meta.dirname, '../src/client')
  for (const name of readdirSync(dir)) {
    const source = readFileSync(join(dir, name), 'utf8')
    for (const match of source.matchAll(/^import (type )?[^'"]*from '(@deepseek-ai\/[^']+)'/gm))
      expect({ name, from: match[2], typeOnly: !!match[1] }).toMatchObject({ typeOnly: true })
    expect(source).not.toContain("dsh-client-ui-primitives'")
  }
})

it('imports each module of the panel only from modules that do not import it back', () => {
  const dir = join(import.meta.dirname, '../src/client')
  const graph = new Map(
    readdirSync(dir)
      .filter(name => /\.tsx?$/.test(name))
      .map(name => [
        name,
        [...readFileSync(join(dir, name), 'utf8').matchAll(/^import [^'"]*from '\.\/([\w-]+\.tsx?)'/gm)].map(match => match[1]!),
      ]),
  )
  const done = new Set<string>(),
    cycles: string[] = []
  const visit = (name: string, path: string[]): void => {
    if (path.includes(name)) {
      cycles.push([...path.slice(path.indexOf(name)), name].join(' → '))
      return
    }
    if (done.has(name)) return
    for (const next of graph.get(name) ?? []) visit(next, [...path, name])
    done.add(name)
  }
  for (const name of graph.keys()) visit(name, [])
  expect(cycles).toEqual([])
})

it('describes the management API in a module the panel can bundle, which imports types only', () => {
  const source = readFileSync(join(import.meta.dirname, '../src/app/api.ts'), 'utf8')
  expect([...source.matchAll(/^import (type )?/gm)].every(match => match[1])).toBe(true)
})

it('closes the dialog on Escape and returns focus to the opener', async () => {
  function Harness() {
    const [open, setOpen] = useState(false)
    return <><svg tabIndex={0} aria-label="svg opener" /><button type="button" onClick={() =>{  setOpen(true) }}>open</button><Modal open={open} onClose={() =>{  setOpen(false) }} title="Title" closeLabel="Close">body</Modal></>
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  const opener = container.querySelector<SVGElement>('svg')!
  const open = container.querySelector<HTMLButtonElement>('button')!
  opener.focus()
  await act(async () => { open.click() })
  expect(document.activeElement?.getAttribute('role')).toBe('dialog')
  await act(async () => { document.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' })) })
  await act(async () => { document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })) })
  expect(document.querySelector('[role="dialog"]')).toBeNull()
  expect(document.activeElement).toBe(document.body)
  act(() => { root.unmount() })
})

it('keeps the focus inside the dialog when its parent re-renders, and Tab and Shift+Tab wrap within it', async () => {
  let rerender = () => {}
  function Harness() {
    const [, setCount] = useState(0)
    rerender = () =>{  setCount(count => count + 1) }
    // Pass a new inline onClose on each render (as polling parents do) to verify the effect does not reset focus.
    return <Modal open onClose={() => {}} title="Title" closeLabel="Close" footer={<button type="button">last</button>}><input aria-label="first field" /></Modal>
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  const field = document.querySelector<HTMLInputElement>('[aria-label="first field"]')!
  field.focus()
  await act(async () => { rerender() })
  expect(document.activeElement).toBe(field)
  const [close, , last] = [...document.querySelectorAll<HTMLElement>('[role="dialog"] button, [role="dialog"] input')]
  const tab = (shiftKey = false) => act(async () => { document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey, cancelable: true })) })
  field.focus(); await tab()
  expect((document.querySelector('[role="dialog"]') ? document.activeElement : undefined)).toBe(field)
  last!.focus(); await tab()
  expect(document.activeElement).toBe(close)
  await tab(true)
  expect(document.activeElement).toBe(last)
  act(() => { root.unmount() })
})

it('moves the tab selection with arrow keys and wraps at the ends', async () => {
  const seen: string[] = []
  function Harness() {
    const [value, setValue] = useState<'a' | 'b' | 'c'>('a')
    return <SegmentedTabs label="pages" value={value} onChange={(next) => { seen.push(next); setValue(next) }} items={(['a', 'b', 'c'] as const).map(item => ({ value: item, label: item, id: `t-${item}`, panelId: 'p' }))} />
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  const list = container.querySelector('[role="tablist"]')!
  await act(async () => { list.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true })) })
  await act(async () => { list.dispatchEvent(new KeyboardEvent('keydown', { key: 'x', bubbles: true })) })
  await act(async () => { list.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true })) })
  await act(async () => { list.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true })) })
  await act(async () => { await new Promise(resolve => requestAnimationFrame(resolve)) })
  expect(seen).toEqual(['c', 'a'])
  expect(container.querySelector('[aria-selected="true"]')?.textContent).toBe('a')
  act(() => { root.unmount() })
})


it('marks the scrollable edges of a tab bar and measures again when its size changes or it is unmounted', async () => {
  const observers: { measure: () => void }[] = []
  class FakeResizeObserver {
    disconnect = vi.fn()
    constructor(measure: () => void) { observers.push({ measure }) }
    observe() {}
  }
  vi.stubGlobal('ResizeObserver', FakeResizeObserver)
  function Harness() {
    return <SegmentedTabs label="pages" value="a" onChange={() => {}} items={['a', 'b'].map(item => ({ value: item as 'a' | 'b', label: item, id: item, panelId: 'p' }))} />
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  const list = container.querySelector<HTMLDivElement>('[role="tablist"]')!
  const setEdges = (scrollLeft: number, clientWidth: number, scrollWidth: number) => {
    list.scrollLeft = scrollLeft
    Object.defineProperty(list, 'clientWidth', { value: clientWidth, configurable: true })
    Object.defineProperty(list, 'scrollWidth', { value: scrollWidth, configurable: true })
    act(() => { observers[0]!.measure() })
  }
  expect(list.dataset.more).toBeUndefined()
  setEdges(0, 10, 20); expect(list.dataset.more).toBe('end')
  setEdges(5, 5, 15); expect(list.dataset.more).toBe('both')
  setEdges(5, 10, 11); expect(list.dataset.more).toBe('start')
  setEdges(0, 20, 20); expect(list.dataset.more).toBeUndefined()
  act(() => { root.unmount() })
  act(() => { observers[0]!.measure() })
  vi.unstubAllGlobals()
})

it('measures a tab bar even where ResizeObserver is unavailable', async () => {
  vi.stubGlobal('ResizeObserver', undefined)
  function Harness() {
    return <SegmentedTabs label="pages" value="a" onChange={() => {}} items={[{ value: 'a', label: 'A', id: 'a', panelId: 'p' }]} />
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  expect(container.querySelector('[role="tab"]')?.textContent).toBe('A')
  act(() => { root.unmount() }); container.remove()
  vi.unstubAllGlobals()
})

it('keeps a disabled segmented control inert for keyboard users', async () => {
  const seen: string[] = []
  function Harness() {
    return <SegmentedControl label="mode" disabled value="a" onChange={(next) => { seen.push(next) }}
      options={[{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]} />
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  await act(async () => { container.querySelector('[role="tablist"]')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true })) })
  expect(seen).toEqual([])
  act(() => { root.unmount() })
})

it('moves an enabled segmented control by keyboard and ignores a click on its active option', async () => {
  const seen: string[] = []
  function Harness() {
    const [value, setValue] = useState<'a' | 'b'>('a')
    return <SegmentedControl label="mode" value={value} onChange={(next) => { seen.push(next); setValue(next) }}
      options={[{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]} />
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  await act(async () => { container.querySelector('[role="tablist"]')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true })) })
  expect(seen).toEqual(['b'])
  const active = container.querySelectorAll<HTMLButtonElement>('[role="tab"]')[1]!
  await act(async () => { active.click() })
  expect(seen).toEqual(['b'])
  act(() => { root.unmount() }); container.remove()
})

it('toggles a disclosure row with Enter and Space, and ignores other keys', async () => {
  const toggled: number[] = []
  function Harness() {
    return <>
      <DisclosureRow icon="+" title="First" open={false} onToggle={() => { toggled.push(1) }}>first</DisclosureRow>
      <DisclosureRow icon="+" title="Second" open={false} onToggle={() => { toggled.push(2) }}>second</DisclosureRow>
    </>
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  const [first, second] = [...container.querySelectorAll<HTMLElement>('[data-disclosure-row]')]
  if (first === undefined || second === undefined) throw new Error('expected two disclosure rows')
  await act(async () => { first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })) })
  await act(async () => { second.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true })) })
  await act(async () => { first.dispatchEvent(new KeyboardEvent('keydown', { key: 'x', bubbles: true })) })
  expect(toggled).toEqual([1, 2])
  act(() => { root.unmount() })
})

it('opens a tooltip above its anchor, supports touch and keyboard dismissal, and closes on scroll', async () => {
  const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    const above = this.className.includes('dt-tooltip')
    return {
      top: above ? 0 : 100, bottom: above ? 10 : 120, left: 40, right: 60,
      width: 20, height: above ? 10 : 20, x: 40, y: above ? 0 : 100, toJSON: () => ({}),
    }
  })
  const focusable = vi.spyOn(HTMLElement.prototype, 'matches').mockImplementation(function (this: HTMLElement, selector) {
    return selector === ':focus-visible' && this.className.includes('dt-info')
  })
  function Harness() { return <InfoTip text="What this means" /> }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  const trigger = container.querySelector('button')!
  await act(async () => { trigger.dispatchEvent(new FocusEvent('focusin', { bubbles: true })) })
  expect(document.querySelector('[role="tooltip"]')).not.toBeNull()
  await act(async () => { trigger.dispatchEvent(new FocusEvent('focusout', { bubbles: true })) })
  await act(async () => { trigger.dispatchEvent(new MouseEvent('mouseover', { bubbles: true })) })
  const pointer = (type: string) => {
    const event = new MouseEvent('pointerup', { bubbles: true, cancelable: true })
    Object.defineProperty(event, 'pointerType', { value: type })
    return event
  }
  await act(async () => { trigger.dispatchEvent(pointer('mouse')) })
  expect(document.querySelector('[role="tooltip"]')).not.toBeNull()
  await act(async () => { trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true })) })
  await act(async () => { window.dispatchEvent(new Event('scroll')) })
  expect(document.querySelector('[role="tooltip"]')).toBeNull()
  await act(async () => { trigger.dispatchEvent(pointer('touch')) })
  expect(document.querySelector('[role="tooltip"]')).not.toBeNull()
  await act(async () => { trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })) })
  expect(document.querySelector('[role="tooltip"]')).toBeNull()
  await act(async () => { trigger.dispatchEvent(new FocusEvent('focusin', { bubbles: true })) })
  await act(async () => { trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 0 })) })
  expect(document.querySelector('[role="tooltip"]')).not.toBeNull()
  const touchClick = new MouseEvent('click', { bubbles: true, detail: 1 })
  Object.defineProperty(touchClick, 'pointerType', { value: 'touch' })
  await act(async () => { trigger.dispatchEvent(touchClick) })
  expect(document.querySelector('[role="tooltip"]')).not.toBeNull()
  const click = new MouseEvent('click', { bubbles: true, detail: 1 })
  await act(async () => { trigger.dispatchEvent(click) })
  expect(document.querySelector('[role="tooltip"]')).toBeNull()
  focusable.mockRestore()
  focusable.mockImplementation(() => false)
  await act(async () => { trigger.dispatchEvent(new FocusEvent('focusin', { bubbles: true })) })
  expect(document.querySelector('[role="tooltip"]')).toBeNull()
  focusable.mockRestore(); rect.mockRestore()
  act(() => { root.unmount() })
})
